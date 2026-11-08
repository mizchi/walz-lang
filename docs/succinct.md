# 簡潔索引とWorkerバッチ

rank/select付き不変BitVectorと、u8値域のWavelet MatrixをWLTで実装した。基礎となる索引は512bitごとのu32 prefix。単独rankはスカラー、Wavelet内部のrankはv128のpopcount・pairwise加算を使う。selectはprefixのupper_boundで対象blockを選び、最大16wordを走査する。空blockの重複prefixと、末尾paddingを数えないselect0を扱う。

実装は[examples/succinct](../examples/succinct)。構築・検証・問い合わせはWLT、Workerの管理は[汎用WorkerHost](worker-host.md)、索引固有の接続は小さな[driver](../runtime/succinct/driver.mjs)に分けた。標準の配列やJSON ASTを全面置換するAPIではなく、繰り返す範囲統計の実験用ライブラリとして提供する。

```sh
just succinct-build target/succinct/library
# 単体APIをComponentにする場合
just succinct-build target/succinct/component component
```

生成物のsdk・workers・serial・reader.wasmを一緒に配布する。開発時にはnative Leanが必要だが、生成物の実行にコンパイラやLeanは不要。

```js
import { instantiate, queries } from './target/succinct/library/sdk/index.mjs';
const sdk = await instantiate(new URL('./target/succinct/library/', import.meta.url));

await using index = sdk.buildWavelet(Uint8Array.of(8, 1, 5, 2, 9, 3));
await index.prepare({ workers: 4 });
const frequency = await index.queryMany(
  queries.rangeFreq([[0, 6, 2, 8]]), // [0,6)内の値[2,8)の個数
  { workers: 4 },
); // Int32Array [3]
const quantile = await index.queryMany(
  queries.quantile([[0, 6, 2]]), // 0-originの2番目
  { workers: 4 },
); // Int32Array [3]
```

BitVectorは`buildBits(bytes, logicalBitLength)`で作る。各byteの下位bitから位置を数え、末尾の未使用bitは0にする。`queries.rank1(ends)`は半開区間[0,end)の1の個数、`queries.select1(ranks)` / `select0(ranks)`は0-originの位置を返す。selectの欠落、空区間や範囲外kのquantileは-1。rankの範囲外、逆順の区間、不正な値域はエラー。Waveletの値域は0〜255、rangeFreqの上限には256を使える。

元データは構築時にコピーする。`snapshot()`も独立したコピーで、変更しても索引に反映されない。共有するのは索引本体・prefix・zero count等の補助表。GC参照は共有しない。問い合わせはWorkerの私有メモリに渡し、結果も私有bufferからtransferで返す。複数のqueryManyも汎用ホストのキューで扱う。各queryMany内の結果順を保ち、AbortSignalで一つのバッチだけを取り消せる。

初回のprepareでWorker数を決める。同じindexではその数を使い続ける。バッチ上限は524,288問い合わせ。初版の共有memoryは16MiB固定で、`byteLength`はそのcapacityではなく索引が使う論理サイズ。private出力メモリは問い合わせ数に応じてgrowする。小さい索引のcapacity削減は今後の調整点。

公開前に一度初期化し、以後は[readonly領域の契約](readonly-regions.md)に従う。close・取消しでは読取り完了の確認を待ち、共有領域を早期に再利用しない。静的なBuilderの部分moveや汎用borrow checkerは追加していない。

## 検査と再計測

`just test-succinct`で、展開した配列から求める独立oracle、同じ索引方式のJS、両単体バックエンド、1/2/4 Worker、Chromium、取消しと終了失敗、領域guard・Lean証明書を検査する。空、全0/全1、疎な列、32/512bit境界、選択の欠落、頻度・分位点を含む。

```sh
just bench-succinct new-run-id ../jsimd
node --expose-gc runtime/succinct/compare.mjs before-id after-id new-comparison-id
```

計測は新しいIDだけを受け付ける。WLTソース、実際のjsimd公開APIとWasm、生成物、入力、compiler hash、Node/V8/CPUと9標本を保存する。同じ索引を持つJSを対照とし、jsimdの線形selectと索引selectのアルゴリズム差を、WLT固有の倍率として扱わない。

[初回スカラー](../benchmarks/succinct/20261108-r1-scalar/report.md)、[SIMD試行](../benchmarks/succinct/20261108-r2-simd/report.md)、[同一プロセスの交互比較](../benchmarks/succinct/20261108-r3-comparison/report.md)、[汎用ホスト版](../benchmarks/succinct/20261108-r4-general-host/report.md)を別々に保存した。

交互比較では、SIMDはrangeFreqを約1.30倍、quantileを約1.06倍にした一方、単独rankは約9%遅くなった。このため単独rankにはスカラーを残した。汎用ホスト版の262,144要素・32,768件では、quantileは同方式JS 11.10ms、jsimd 6.16ms、WLT local 6.14ms、WLT 4 Worker 1.69ms。rangeFreqはJS 18.90ms、jsimd 13.49ms、WLT local 11.11ms、4 Worker 2.92msだった。

これらは構築・起動を除き、問い合わせコピー・結果取得とWorker通信を含むバッチ中央値。小さい単発rankではJSが速い。Worker準備はこの実行で十数〜数十msあり、常駐させて複数バッチに償却する必要がある。build/prepare/初回queryは別に記録し、損益分岐バッチ数はその一回のsetup値と定常中央値からの概算である。完全なcold-start分布やp95を測った値ではない。

測定条件はApple M5・Node 24.21.0/V8 13.6、jsimd `2cf51e8`。u8全値域のWaveletは約1.063 bytes/valueであり、Uint8Arrayより小さいという主張はしない。固定比率の補助表を持つ実装で、数学的なn+o(n)やentropy boundの達成を証明したものでもない。
