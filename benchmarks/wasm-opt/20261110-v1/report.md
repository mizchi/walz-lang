# Binaryen 133 に合わせた match lowering

通常の open-world `-O3` で最適化できる出力を調べ、8ケース以上の enum match を
タグ比較の列から `br_table` に変えた。最適化後の全体サイズを約7〜11%、code sectionを約11〜18%削減した。
32ケースの混合タグ読み出しはV8で約5%速くなったが、8ケースでは約2%遅くなった。
速度とサイズの両方を一律に改善する変更ではない。

## 記録と比較条件

- before は `9b8a8b31030b9bed9d8a09e565a94348adbfb018` のコンパイラ。
- after は同じ実装に [emitter.patch](emitter.patch) を適用したもの。
- [before の manifest](before/manifest.json) と [after の manifest](after/manifest.json) の実装ハッシュの差は `src/emit.rs` だけ。固定した6入力と計測ツールのハッシュは一致する。
- 実行環境は Apple M5 / macOS 26.6.2 / Node v24.22.2 / Wasmtime 42.0.1。バイナリと公式配布物のSHA-256は [toolchain.json](toolchain.json) に記録した。
- Binaryen は [version_133 の公式配布](https://github.com/WebAssembly/binaryen/releases/tag/version_133)。通常の `-O3` と `-O3 -O3` を使い、GCとreference-typesを有効にした。closed-world、GUFA、trapを無視する設定は使っていない。
- 入力・生成物・metrics・コマンド・生の測定値は [before](before/result.json)、[after](after/result.json) に保存した。入力のarmは宣言順の逆にしてある。

最初のcaptureは局所structのfixtureに `allocates` 注釈がなく、コンパイラが拒否した。
[失敗したrun](failed-first-capture/failure.log)は残したまま、fixtureを修正し、コンパイラを変える前に別IDでbeforeを取り直した。
失敗したrunの数値は比較に含めていない。

## すでに最適化できていたもの

| 固定入力 | 生の全体サイズ | `-O3` 全体サイズ | 生のcode section | `-O3` code section |
| --- | ---: | ---: | ---: | ---: |
| 不要な定数束縛 | 76 | 37 | 18 | 6 |
| 局所structの構築・読み出し | 101 | 37 | 23 | 6 |
| 局所Resultの構築・match | 232 | 37 | 84 | 6 |

単位はバイト。3つとも最適化後は `main` の `(i32.const 42)` だけになった。
afterでも生成Wasmはbeforeと同じである。これらの処理をWLT側に追加実装する必要はなかった。
非escape割当ての除去、定数伝播、不要なローカルの削除という観測であり、任意のオブジェクトの割当て除去を保証しない。

## 大きな enum match

外部に公開した `read(ref E)` と各variantのmakerを残したため、`main`だけに定数畳み込みできない入力で比較した。
payloadの読み出しとcase固有の整数加算を含み、castと不正タグのtrapを残している。

| ケース数 | `-O3` 全体 before→after | 全体の差 | code section before→after | codeの差 |
| --- | ---: | ---: | ---: | ---: |
| 8 | 496→463 | −6.65% | 292→259 | −11.30% |
| 16 | 928→839 | −9.59% | 572→483 | −15.56% |
| 32 | 1792→1591 | −11.22% | 1132→931 | −17.76% |

afterに `-O3 -O3` を使うと、それぞれ458／834／1586バイトになった。追加の削減は各5バイト。
beforeでは2回目を回しても同じWasmだった。
GC言語で複数回のパイプラインが有用になるという [公式ガイド](https://github.com/WebAssembly/binaryen/wiki/GC-Optimization-Guidebook)の例とも一致するが、追加の処理時間との判断が必要になる。

生のWLT出力では、全体サイズは881→1007、1654→1866、3206→3594バイトと増える。
一方code sectionは339→295、651→551、1275→1063バイトへ減る。
ラベル名を含むname sectionが増えるためで、デバッグ名を落とすBinaryen後の全体サイズと区別する。

小さいmatchは従来の比較列を使う。
[探索時の2／4／6ケース](exploratory-small-tables/manifest.json)では、手でテーブル化した出力のサイズ差はそれぞれ+9／−5／−19バイトだった。
小さいテーブルの固定負担と小さな削減量を踏まえ、初期のしきい値は8とした。
探索用の入力は順方向armで、上記の固定した逆順armの計測とは別に保存している。
このしきい値がすべてのエンジンで最速という意味ではない。

## 速度

capture内のbeforeとafterの測定は時刻が異なる。
速度の比較には、保存済みのWasmを同じNodeプロセスで交互に測った
[comparison.json](comparison/comparison.json)を使う。
GC参照を1024個事前に構築し、固定seedの混合タグを読む。各呼出しの結果42と、全caseの境界値・折り返しを照合した。
100,000呼出しでwarmupし、1,000,000呼出し×15組をbefore→after／after→beforeと交互に測った。
時間にはJS→Wasmの呼出しを含む。構築・コンパイル・インスタンス化は含めない。

| ケース数 | `-O3` before 中央値 ms | after 中央値 ms | 対応する時間比 after/before の中央値 |
| --- | ---: | ---: | ---: |
| 8 | 19.427 | 20.172 | 1.0210（約2%悪化） |
| 16 | 21.994 | 21.951 | 1.0009（ほぼ同等） |
| 32 | 23.688 | 22.600 | 0.9536（約5%改善） |

表の比は各組の比の中央値であり、両列の中央値を割った値ではない。
rawと2回の`-O3`の測定も同じJSONに残している。外れ値は除いていない。
Wasm内の長い処理、実アプリ全体、Wasmtimeでの速度はこの表から推定しない。

## 意味の維持と再実行

生成時のテーブルは宣言タグ順、armの本体配置はソース順にする。
subjectの評価とタグ読み取りは各一回、選択したvariantへのcastは省略しない。
nullary caseもcastを行い、負のタグ・上限以上のタグは `unreachable` へ進む。

追加テストは8〜64ケース、逆順arm、ネスト、副作用の一回評価、return、外側labelへのbranch、
例外、unit、所有値を返すarmを扱う。標準Wasmからbase値やタグと異なるsubtypeを渡す場合のtrapも照合する。
Binaryenを通した後も、nullary caseと未使用の整数除算によるtrapをWasmtime／V8で確認する。
Core＋JSPI／ComponentのABIとChromiumのブラウザコンパイラは別のテストで確認した。
形式証明の対象を広げたものではなく、emitterは引き続きTCBに含まれる。

```sh
just test-wasm-opt /path/to/binaryen-133/bin/wasm-opt
just wasm-opt-capture fresh-capture /path/to/binaryen-133/bin/wasm-opt
just wasm-opt-compare \
  benchmarks/wasm-opt/20261110-v1/before \
  benchmarks/wasm-opt/20261110-v1/after fresh-comparison
```

計測ツールの当時の内容は [tools](tools/wasm-opt-capture.mjs) に保存した。
通常はリポジトリ直下の同名ツールをjust経由で実行する。
再実行では既存runを上書きせず、新しいIDを使う。
WLTのビルドにBinaryenの依存は加えていない。

出力設計と一次資料の対応は [wasm-opt の説明](../../../docs/wasm-opt.md)を参照。
