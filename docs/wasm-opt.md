# wasm-opt に合わせた Wasm GC の出力

Binaryen 133 の通常の `-O3` を基準に、WLT の出力を調べた。
外部に公開した GC 参照と trap の意味を保ったまま、大きな `%match` をテーブル分岐へ変えている。
[固定した入力・生成物・計測結果](../benchmarks/wasm-opt/20261110-v1/report.md)を参照。

## どの形が最適化されるか

| 入力の形 | Binaryen の処理 | WLT の方針 |
| --- | --- | --- |
| 定数演算、不要な純粋な束縛 | 定数伝播・畳み込み、不要なローカルの削除 | 数値命令とローカルとして出力する |
| 関数内で作って読み出すだけの GC struct／Result | `heap2local` で割当てをローカルへ置き換え、後続パスで簡約できる | `struct.new`／`struct.get` と具体的な型を維持する |
| 既知の型への cast、不変フィールドの読み出し | `optimize-casts`、`local-subtyping`、共通式の簡約 | 非 null 型・不変フィールド・具体的な subtype を保持する |
| 直接呼出し、到達不能な関数 | インライン化、未使用要素の削除 | 静的な呼出し先と明示的な export を保持する |
| タグを順番に比較する大きな match | 今回の入力では `-O3` 後も比較列が残った | 8ケース以上は `br_table` を出す |

GC の標準パスを含む処理順は [133 のパス定義](https://github.com/WebAssembly/binaryen/blob/version_133/src/passes/pass.cpp)で確認した。
具体的な型を残すことや、不変フィールドを使う方針は
[GC の lowering 指針](https://github.com/WebAssembly/binaryen/wiki/GC-Implementation---Lowering-Tips)にも沿っている。
割当て除去は参照が外へ渡らず、同一性などの観測を保てる場合に限られる。
任意の GC オブジェクトや、ホストへ渡る ABI 値が消えるという保証ではない。

固定した3つの小例では、不要な束縛、局所 struct、局所 Result がいずれも
`(i32.const 42)` だけの関数になった。最適化後は各37バイト、code section は各6バイト。
この部分は既存の出力で最適化できるため、WLT に重複するパスを追加していない。

## 大きな match の出力

検査済み enum のタグは宣言順に `0..N-1` で、match は網羅的になる。
8ケース以上では subject を一度だけ一時ローカルに保存し、不変のタグを一度だけ読み、
`br_table` で選んだ case へ進む。テーブル順は宣言タグ順なので、ソースの arm 順に依存しない。
7ケース以下の出力は従来の `if` 列を保つ。小さいテーブルには固定の負担があるため、8を初期のしきい値とした。
この出力選択は lowering の一部で、WLT の `--no-optimize` でも使われる。

各 case には従来どおり具体的な subtype への `ref.cast` を残す。
payload のない case も cast を実行する。範囲外のタグは `unreachable` へ進む。
これにより、外部 Wasm が作った「base 型だけの値」「タグと subtype が違う値」を受け入れない。
関数の評価順、return、外側への branch、例外、unit、所有値を返す arm の扱いも維持する。

`-O3` 後の全体サイズは、8／16／32ケースでそれぞれ496→463、928→839、1792→1591バイトになった。
code section は292→259、572→483、1132→931バイト。
生の WLT 出力はラベル名を含む name section が増えるため、全体サイズが増える場合がある。
命令部分とデバッグ名を含む全体サイズを分けて比較する。

速度はサイズと同じ傾向とは限らない。V8で事前構築した参照を混ぜて読む比較では、
32ケースは約5%改善、16ケースはほぼ同等、8ケースは約2%悪化した。
JSからの呼出し、タグ分布、JIT、CPUに依存する小規模な測定であり、実アプリ全体の高速化を保証しない。

## 任意の後処理として実行する

```sh
wlt build input.wlt --target core -o target/input.wasm
wasm-opt target/input.wasm \
  --enable-gc --enable-reference-types --enable-exception-handling \
  -O3 -o target/input.opt.wasm
```

使う Wasm 機能に応じてフラグを指定する。上記は Core Wasm 用で、Component 全体を直接処理する手順ではない。
WLT の通常ビルドと Playground は `wasm-opt` を起動しない。Rust 製ブラウザコンパイラ自体の wasm-pack 設定とも別の話になる。

GC 向けには複数回のパイプライン実行が効くことがあり、
[公式ガイド](https://github.com/WebAssembly/binaryen/wiki/GC-Optimization-Guidebook)は `-O3 -O3` も紹介している。
今回の変更後の enum では、2回目でさらに5バイト小さくなった。

通常の検査・計測では `--closed-world`、GUFA、`--traps-never-happen`、
`--ignore-implicit-traps`、fast-math を指定しない。
closed-world は外部が GC 参照をどう扱うかに制約を加え、trap を無視する設定は不要な式の削除条件を変える。
公開参照や除算の trap を保持する今回の比較とは異なる前提になる。
設定の契約は [133 のオプション定義](https://github.com/WebAssembly/binaryen/blob/version_133/src/tools/optimization-options.h)を参照。

## 最適化しにくいソースへの任意警告

```sh
wlt check examples/optimization/loop-copy.wlt --warn-performance
wlt check examples/optimization/loop-copy.wlt --warn-performance --json
just check-performance examples/optimization/loop-copy.wlt
```

通常の型・effect・所有権・race・lawの検査を通した後、型付きIRの根拠から改善候補を出す。
コンパイルエラーがあればそちらを返す。警告だけでは終了コードを失敗にしない。
既定では無効で、ソースの自動修正や生成Wasmの変更は行わない。

| コード | 検出する形 | 検討する変更 |
| --- | --- | --- |
| `W_OPT_LOOP_COPY` | ループ本体のbytes／listのslice、またはそれを行う静的な呼出し先 | 元データと添字・範囲を保持する、bytes/string viewを使う、意味を保てる場合だけループ外へ移す |
| `W_OPT_LOOP_HOST_ESCAPE` | 同じ関数のループ本体で割り当てたGC値がホストへ逃げる可能性 | ホストの契約に合うならスカラーを渡す、Wasm内にGC値を保つ、境界でまとめて転送する |

copyの判定は関数名ではなく検査済みの操作と呼出しグラフを使い、importの別名や具体化した関数にも対応する。
ヘルパー越しのコピーには実際のsliceの位置も関連情報として示す。
host escapeは既存の領域解析に基づく保守的な判定で、ホスト内の処理までは証明しない。
ヘルパー内の割当てが呼出し元のループで繰り返されるケースなど、すべての反復経路を網羅するものではない。

ループの初期値だけで行うコピー、未参照の宣言、解析で到達不能と分かった箇所は除外する。
外側のループで繰り返される内側ループの初期化は対象になる。
可変フィールドやnullable型があるという理由だけでは警告しない。
マクロ展開で同じソース位置に同じ警告が重なった場合はまとめる。

[loop-copy.wlt](../examples/optimization/loop-copy.wlt)は、毎回sliceしたbytesの先頭を読む例。
[loop-indexed.wlt](../examples/optimization/loop-indexed.wlt)は、同じ不変データを直接読む例になる。
Binaryen 133の通常の `-O3` では、前者に `array.copy` が残り、後者では消える。
最適化後の全体サイズは253→214バイト。空データで反復0回なら0を返し、1回ならtrapする性質も両方で照合している。
入力・診断・生成物は [性能警告の確認記録](../benchmarks/wasm-opt/20261110-advice-v1/report.md) に保存してある。
これはソースを明示的に書き換えた例で、一般のコピーを安全に削除できるという判定ではない。

警告はBinaryen自身の「最適化失敗」通知や、計測したホットスポットではない。
コピーが必要なsnapshotである場合や、ホストがオブジェクト同一性を必要とする場合もある。
ループの頻度、境界trap、変更の観測、データ保持期間、ホストABIを確認して採否を決める。
実際の最適化結果・プロファイルを調べる入口として使う。

`--json` では従来の `wlt-check-v1` に `performance` を追加する。
`diagnostics` はエラー用、`performance.warnings` は既存のDiagnostic形式の警告用で、混在させない。
`performance.schema` は `wlt-performance-v1`、`profile` は根拠とした `binaryen-133-open-world`。
これはインストール済みのoptimizerを検出した結果ではなく、外部の実行ファイルは起動しない。
`coverage: "opaque_wat"` は標準WATの式レベルの解析ができないことを表し、「改善候補なし」という意味ではない。
フラグなしの `check --json` は従来の形式を保つ。

Rust APIは `performance::check_with_policy(source, policy)`。
検査済みIRを再利用する場合は `performance::analyze(source, module)` を使える。
読み込んだ複数ファイルの位置へ戻すには `Report::located(&source)` を呼ぶ。
解析は入力IRを変更せず、出力先に依存しないため、Core＋JSPIやComponent用のtaskもCore出力を要求せずに検査できる。

LSPでは `initializationOptions: {"performanceWarnings": true}`、または
`workspace/didChangeConfiguration` の `settings: {"wlt": {"performanceWarnings": true}}` で有効にする。
severityは2（Warning）。元のファイル位置とUTF-16範囲、ヘルパーの関連位置を表示する。
同じrevisionでは計算結果を再利用し、無効化・編集・クローズで古い警告を消す。
LSPは従来どおり型付きsnapshotから診断するため、lawの証明にはCLIのcheck/buildを使う。

## 再実行

`wasm-opt --version` が133である実行ファイルを明示する。別の版で調べる場合は課題・記録も別版にする。

```sh
just test-wasm-opt /path/to/binaryen-133/bin/wasm-opt
just test-performance-warnings
just wasm-opt-capture new-run /path/to/binaryen-133/bin/wasm-opt
just wasm-opt-compare \
  benchmarks/wasm-opt/20261110-v1/before \
  benchmarks/wasm-opt/20261110-v1/after new-comparison
```

capture は `target/wasm-opt/<run>/` に入力、WAT、Wasm、最適化後の命令と metrics、
コンパイラ実装・入力の SHA-256、実行コマンド、生の測定値を保存する。既存 run を上書きしない。
compare は両側の入力とプロファイルの一致、保存した入力とWasmのハッシュ、全caseの値を検査し、
同じプロセスで before／after の順を交互に変えて測る。

通常の Rust テストは8〜64ケース、arm の並べ替え、ネスト、副作用の一回評価、分岐と所有値を検査する。
`test-wasm-opt` は133を使い、最適化後の値・不正タグ・不正subtype・nullary case・未使用の除算trapを
WasmtimeとV8で確認する。Core／ComponentのABI検査とPlaygroundのE2Eも別途持つ。
性能警告の検査は、正例・除外例、importとマクロの位置、JSONとLSPの任意有効化に加え、
上記のコピー有無とホストが観測するGC参照の同一性もBinaryen後に確認する。
