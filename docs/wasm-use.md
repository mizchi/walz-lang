# WIT付きComponent Wasmの%use

`%use`は`.wlt`に加え、WITの型を持つ実行可能なComponent `.wasm`を読める。exportの引数・戻り値をバイナリから取り出し、WLTの型へ変換する。別のWITファイルを実行時に探したり、Core Wasmの整数ポインタからABIを推測したりはしない。

```wat
(module
  (%use "./math.wasm" (func $math.add $add))
  (func $main (export "main") (result i32) (%effect host)
    (call $add 20 22)))
```

WIT worldが`math` interfaceをexportする場合、関数名は`$math.add`。world直下の関数なら`$add`を使う。別名・`.wlt`による再exportは通常のファイルモジュールと同じ。名前の衝突は拒否する。

## ライブラリを作る

`math.wit`:

```wit
package example:math;
interface math {
  add: func(a: s32, b: s32) -> s32;
}
world library { export math; }
```

`math.wlt`:

```wat
(module
  (func $add (export "math-add") (param $a i32) (param $b i32)
    (result i32) (%effect pure) (+ $a $b)))
```

```sh
wlt build math.wlt --target component --wit math.wit --world library -o math.wasm
wlt check main.wlt
wlt build main.wlt --target component -o main.component.wasm
# Node／ブラウザ用の生成JSと一緒に配布する場合
node runtime/build.mjs main.wlt target/main
```

JSからは`const {api} = await instantiate(); const value = await api.main();`のように待つ。WITでは同期関数でも、Component間の呼出しを処理するJSPI adapterがPromiseを返す場合がある。ホストは実際のasync関数を待ち、完了前に次のジョブを開始しない。

`--wit`で元のworldに沿ったComponentを作る。通常のWLT Component出力は内部delivery importを持つため、再利用するライブラリにはこの形式を使う。WIT metadataを付けただけのCore Wasmや、型定義だけのWIT packageは実行可能なComponentではない。Core WasmはCanonical ABIのwrapperを持つComponentへ変換してから渡す。

## 型・effect・所有権

初版では**ホストimportのないComponentの、同期・共有値の関数**を扱う。

- s32/s64/f32/f64、文字列、bytes、共有要素のlist、record、enum、flags、tuple、option/resultなど、既存のWIT value adapterで表現できる型を使う。
- unsigned・bool・charなどは既存のWIT bindingと同じnominal型になる。必要なrecordやunsigned型は、例えば`(type $math.point $Point)`・`(type $wit-u32 $Count)`で明示的に取り込める。
- 型検査・effect・opaque・所有権の既存検査を通す。WITには純粋性の証明がないので、外部呼出しは保守的に`host` effectを持つ。`pure`として扱わない。
- async関数と、Future・Stream・resourceを含む引数／結果は、選択した位置で`E_WASM_USE`になる。入れ子の型にも適用する。同じバイナリ内にそれらがあっても、対応する同期exportは使える。

Component境界を越えるFutureの取消しは、現状のJS実行系ではWLTの取消し例外から通常のtrapへ変わる場合がある。WITの型情報だけでは取消し・join・端点の寿命を定められないため、これらは境界契約が整うまで受理しない。既存のWLT内のFuture／Streamや、明示的なホストbindingの対応範囲は変えない。

## リンクと実行

読み込み時にバイナリもsnapshotする。同じcanonical pathを何度取り込んでも、到達する依存は一つのComponentインスタンスになり、内部状態を共有する。別ファイルは別の依存である。

生成Componentには元のバイナリをそのまま埋め込み、Canonical ABIで値を渡す。Wasm GC参照や生のmemoryを別インスタンスへ渡さない。元の`.wlt`・`.wasm`・`.wit`を削除しても、生成した配布物は実行できる。内部依存をホストimportの一覧には出さない。

未使用の関数bindingをDCEし、依存全体に到達しなければ埋込み・インスタンス化も除去する。使う依存の内部コードを関数単位でDCEする機能ではない。到達する依存の初期化・startは通常のComponent実行に従い、型検査中には実行しない。

現時点の制限:

- 出力は`--target component`。Core／Core＋JSPIへの出力は`E_COMPONENT_USE_BACKEND`で拒否する。依存がDCEで消えればCore出力もできる。
- importのあるComponentは、必要なcapabilityを明示的に合成して閉じてから渡す。未知のホスト機能を自動で許可しない。
- `.wasm`には`%use`を使い、`%include`は使わない。単体4MiB、展開後のソース全体16MiBの上限がある。
- WITの型にも展開量・深さの上限があり、共有する型グラフを巨大な文字列へ展開する前に検査する。
- 型をWITから得られても、外部実装の純粋性、停止性、内部の資源解放を証明したことにはならない。`%race-free`は外部関数を引き続き拒否する。

`just test-wasm-use`で、top-level／interface export、別名・再export・同一インスタンス、型・effect拒否、DCE、元ファイル不要の実行、ABI領域の解放、未実行ジョブの取消し、未対応境界の拒否とChromiumを検査する。[実行fixture](../tests/component-fixtures/wasm-use)を参照。
