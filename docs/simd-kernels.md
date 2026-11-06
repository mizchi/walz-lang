# SIMD カーネルと速度の比較

[jsimd のREADME](https://github.com/mizchi/jsimd/blob/main/packages/jsimd/README.md)から、i32集計、Float32内積、bitmap交差件数、bytes検索の4処理をWLTで実装しています。データ構造全体や全メソッドの移植ではなく、GC上のSIMDが実用的な速度になるかを調べるための例です。

| 処理 | WLTの関数 | 値の規約 |
| --- | --- | --- |
| i32の合計 | `sum-scalar` / `sum-simd` | 符号付きi32をi64へ拡張して加算。i32の桁あふれを起こさない |
| Float32内積 | `dot-scalar` / `dot-simd` | 同じ長さ。スカラーは逐次f32加算、SIMDは4laneに加算して集約 |
| bitmap交差件数 | `bitmap-scalar` / `bitmap-simd` | 同じ長さの32bitワード列。ANDしたビットのpopcount |
| byte検索 | `bytes-scalar` / `bytes-simd` | 最初の一致位置、見つからなければ−1。needleは0〜255 |

## 実装と使い方

[kernels.wlt](../examples/simd-kernels/kernels.wlt) は線形メモリを使わず、通常のGC配列と `array v128` を使います。`pack-i32` / `pack-f32` は4要素、`pack-bytes` は16byteずつ詰め、不完全な末尾をゼロで埋めます。検索では論理長を保持し、パディングのゼロを一致として返しません。パッキング結果は元のスカラー配列から独立したスナップショットです。

整数lane、float lane、byte laneはそれぞれ別のnominal型です。内部の可変GC配列をopaqueにし、WLTの呼び出し側から表現を操作できないようにしています。型・effect・所有権を検査したコアだけを通し、SIMD命令へ直接出力します。新しい言語構文やランタイムは追加していません。必要になった標準Wasmの `i32/i64.popcnt` / `ctz` / `clz` も型付きコアで扱います。これらのlawモデルと値域の精密化は追加していません。

WLT内での使用例は [demo.wlt](../examples/simd-kernels/demo.wlt)。`just run examples/simd-kernels/demo.wlt` は42を返します。配列を変更したら再びpackし、同じpack済みデータを何度も処理する場合は結果を保持してください。

[portable.wlt](../examples/simd-kernels/portable.wlt) はlist/bytesを受け付け、スカラーまたはSIMDで計算して値を返します。Core＋JSPIとComponentの両方に出力できます。

```sh
just build-core-js examples/simd-kernels/portable.wlt target/bulk
```

```js
const { instantiate } = await import("./target/bulk/index.js");
const { api } = await instantiate();
api.sumSimd(new Int32Array([10, 20, 12]));             // 42n
api.dotSimd(new Float32Array([1, 2, 3, 4, 5]),
  new Float32Array([2, 4, 6, 8, 10]));                // 110
api.bitmapSimd(new Int32Array([-1, 15, 0, 8, 1]),
  new Int32Array([3, 7, 9, 8, 1]));                   // 7
api.indexSimd(new Uint8Array([1, 2, 255]), 255);      // 2
```

`sum` の戻り値はi64なので、JSではBigIntです。Float32の丸めと加算順序はWasmの意味に従い、逐次のJavaScript double加算と一般には一致しません。長さの不一致や不正なneedleはtrapです。Componentインスタンスはtrap後に再利用せず、回復可能な失敗が必要なら呼び出し前に検査するかResultで表すラッパーを使います。

## 測定方法

以下の費用を分けて測ります。

- **resident**: 入力を一度GC配列に作り、pack済みの値を保持。JSとの呼び出しと結果の受け取りを含み、初期化・pack・WIT変換は含めない。
- **pack＋計算**: 既存のスカラーGC配列から毎回packして計算。GC割当て・回収も含む。
- **コピー込み**: JSのTypedArrayから、実際のCore＋JSPI／Componentの値アダプター、GC配列への変換、pack、計算、ABIバッファ解放までを含む。モジュールのロードとコンパイルは含めない。

residentの比較用 [resident.wlt](../examples/simd-kernels/resident.wlt) はCore WasmのGC参照をJSにopaqueなオブジェクトとして返し、同じインスタンスへ渡し直します。初期データはsetterで転送し、その費用をresident測定から除きます。これはWITの値ABIとは別のCore用インターフェースです。`v128` 自体をJSへ返しません。スカラーのbyte参照実装ではGCのi32配列を使い、SIMDではbyteを128bitに詰めるため、メモリ密度の差も測定に含まれます。

jsimdはローカルcheckoutの実際の生成済みWasmを保存して比較します。residentは公開JSラッパーを通さない生の線形メモリカーネル、コピー込みは事前確保したscratchへの `TypedArray.set` とカーネルの合計です。公開ライブラリのallocatorや引数検査は含めません。特に実際のjsimdのbytes APIは小さい入力・先頭の一致をJSで処理するため、この比較だけで公開API全体の速さを判断しないでください。

JS参照はTypedArrayのforループ、SWAR popcount、`Uint8Array.indexOf`です。Float32のベンチマーク入力は、小さい整数値で加算順序による誤差が出ない範囲を使います。小数の丸めを含む正しさは別のruntimeテストで確認します。

同じ入力で全実装の答えを検査し、ウォームアップと反復数の調整後に測定順を回転させ、9標本の中央値・最小・最大を保存します。サイズは16、1,024、4,096、16,384、262,144要素。bytes検索はmiss・末尾一致・先頭一致を分けます。帯域・キャッシュ・JIT・GCの影響を含む、この環境での測定です。

## 測定結果（2026-11-06）

Apple M5、Node v24.21.0 / V8 13.6。9標本の中央値、単位はµs/呼び出しです。参照はjsimdのcommit `2cf51e8` のローカルcheckoutにあった生成済みバイナリを保存し、ソースとバイナリを別々にハッシュしています。

16,384要素を保持した場合:

| 処理 | JS | WLT scalar | WLT SIMD | jsimd kernel | WLT SIMDの対JS速度比 |
| --- | ---: | ---: | ---: | ---: | ---: |
| i32合計 | 8.719 | 5.558 | 1.902 | 1.892 | 4.58x |
| Float32内積 | 10.349 | 10.182 | 2.537 | 2.536 | 4.08x |
| bitmap交差件数 | 16.000 | 7.196 | 2.878 | 6.001 | 5.56x |
| bytes検索・miss | 3.786 | 4.955 | 0.534 | 0.458 | 7.09x |

GC上でも、保持したベクトルのカーネルは十分速くなっています。i32合計と内積は参照カーネルとほぼ同速でした。262,144要素でも対JSで3.76〜7.32倍です。一方、16要素ではJSが速い処理があり、bytesの先頭一致ではサイズに関係なくJSが速いままでした。

同じ16,384要素を毎回JSから渡す場合:

| 処理 | JS | Core＋JSPI SIMD | Component SIMD | jsimd scratch copy＋kernel |
| --- | ---: | ---: | ---: | ---: |
| i32合計 | 8.719 | 561.776 | 23.695 | 2.742 |
| Float32内積 | 10.349 | 1136.948 | 42.821 | 4.891 |
| bitmap交差件数 | 16.000 | 1114.953 | 43.416 | 7.746 |
| bytes検索・miss | 3.786 | 17.875 | 20.166 | 0.604 |

現状のportableな単発呼び出しは、SIMDにしてもJSより遅くなります。既存GC配列から毎回packするだけでも、内積は11.126µs、bytes検索は5.581µsかかり、どちらもJSを上回りました。SIMDの効果を得るには、pack済みデータを保持して複数回使うことが必要です。

この初回測定時のCore＋JSPIのlist入力は、要素ごとに値codecとDataViewを通っていました。スカラー版もSIMD版も時間がほぼ同じで、同じカーネルを使うComponentとの差も大きいため、境界の転送を改善しました。

4処理のスカラー／SIMDとresident用コンストラクタをまとめたCore Wasmは3,456 bytes（gzip 1,391 bytes）でした。portable版はWIT/ABI・GC変換を含むため別の大きさです。機能別のjsimdモジュールとbundleサイズを直接比較する値ではありません。

全サイズ・生の標本・入力ハッシュは [report.json](../benchmarks/simd-kernels/20261106-r1/report.json)、読みやすい一覧は [report.md](../benchmarks/simd-kernels/20261106-r1/report.md)、生成物とソースのハッシュは [capture.json](../benchmarks/simd-kernels/20261106-r1/capture.json) に保存しています。元の実行可能なsnapshotは `target/simd-kernels/20261106-r1` にあります。

## 数値listの一括転送後（2026-11-06）

Core＋JSPIで、対応するTypedArrayからの数値list入力と数値list出力を一括コピーするようにしました。通常のArrayの値検査、リストの上限とbounds、解放規則は維持しています。詳細は [list ABI](lists.md) を参照してください。

保存した変更前後の出力を交互に9標本ずつ測定しました。Wasmバイナリ、共通ホストランタイム、入力、計測関数が同一であることをハッシュで検査し、JS側の生成された型変換だけの差を測ります。以下は入力コピー・GC変換・pack・SIMD演算・解放を含む16,384要素の中央値、単位はµsです。

| 処理 | Core変更前 | Core変更後 | 速度比 | Component変更後（対照） |
| --- | ---: | ---: | ---: | ---: |
| i32合計 | 612.414 | 22.187 | 27.60x | 25.023 |
| Float32内積 | 1122.943 | 41.715 | 26.92x | 45.199 |
| bitmap交差件数 | 1221.011 | 42.870 | 28.48x | 45.482 |
| bytes検索・miss（対照） | 18.309 | 18.199 | 1.01x | 21.384 |

TypedArray入力の3処理は、4,096要素で23〜26倍、262,144要素でも19〜21倍速くなりました。通常のArrayを渡す経路は、要素ごとの検査を残しているため同程度の時間です。bytesとComponentは変更していません。大きな入力ではGC割当てを含む対照側にもばらつきがあり、各標本の最小・最大も記録しています。

この変更はJSと線形メモリの間の転送を改善します。GCへの変換とpackは残るため、今回の単発呼び出しはまだJSのスカラーループより遅いままです。次の改善対象は中間GCコピーの削減とpack済みデータの再利用です。

生の標本・入力ハッシュ・比較コードと変更前後のmanifestは [比較結果](../benchmarks/simd-kernels/20261106-r2-bulk/report.md) と [report.json](../benchmarks/simd-kernels/20261106-r2-bulk/report.json) に保存しています。初回のr1は変更していません。

## 再実行

```sh
just test-simd-kernels
pnpm exec playwright test runtime/e2e/simd-kernels.spec.mjs
just simd-kernels-capture target/simd-kernels/my-run ../jsimd
just simd-kernels-bench target/simd-kernels/my-run
# 対応するTypedArrayを使い、保存した変更前後の型変換を交互に測る。
just simd-kernels-compare target/simd-kernels/20261106-r1 \
  target/simd-kernels/20261106-r2-bulk target/simd-kernels/my-comparison
```

参照側の `kernels.wasm` はjsimdで先にビルドしてください。captureは既存ディレクトリを上書きせず、WLTソース・出力、jsimdのバイナリとソース、測定コードをハッシュとともに保存します。runは保存した測定コードを実行し、改変を検出したら拒否します。結果は各実行の `results/<timestamp>/report.json` / `report.md` に新しく保存します。入力のseed・内容のハッシュ、Node/V8・CPU、参照checkoutのcommit、生成物のサイズも記録します。
