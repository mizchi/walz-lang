# Rustフロントエンドの計測

Criterion 0.8.2を開発依存として使い、WLTの型検査とCore Wasmへのコンパイルを測る。
Cargo／rustc自身のビルド時間とは分ける。Rustのbench profileを使い、Wasmの実行は含めない。

初回の改善結果は[20261107-r1の計測記録](../benchmarks/frontend/20261107-r1/report.md)を参照。

```sh
just test-frontend
just frontend-save before-r1
# Rust側を修正し、機能検査を通す
just frontend-compare before-r1
just frontend-bench '^check/std-types$'
```

`frontend-save` は既存のラベルを上書きしない。比較は基準値を維持し、結果は既定で
`target/criterion/frontend-v4/` に保存する。`CRITERION_HOME` で基点を変えても、その下の`frontend-v4/`に分ける。
直接使うときのオプションは[Criterionのbaseline資料](https://bheisler.github.io/criterion.rs/book/user_guide/command_line_options.html)を参照。

## 対象と条件

`benches/fixtures/frontend-v4/` の入力を固定する。`wlt:core`・`%use`・ファイルモジュールへの変更に合わせた新版で、v1〜v3の入力と過去の計測記録は保持する。
大きな入力はファイル解決後のコンパイラ内部形式を固定したもの。非公開のスコープ表現を含むため、CLIの参照比較では明示的にlegacy fixture経路を使い、公開ソースの文法例には使わない。
アプリのファイルimport／includeは事前に展開してあり、
APIの計測にはファイル読み込みを含めない。入力を変更する場合は新しい版を作る。

| group | 対象 |
| --- | --- |
| `check` | 名前・型・effect・所有権の検査。Native WAT、let、標準型、文字列、JSON、在庫台帳、workflow |
| `compile` | 上記の検査、解析、最適化・DCE、WAT／Wasm生成、Wasm validator |
| `check-error` | 標準ライブラリを参照する型エラーの診断 |
| `cold-check` | 毎回新しいCLIプロセスでcheck。起動・ファイルI/Oを含む。OSのファイルキャッシュは温まる |

既定は30サンプル、ウォームアップ500ms、目標計測時間2秒。入力と結果を `std::hint::black_box` に渡し、
生成したIR等の解放も1回の時間に含める。測定設定はCLIで変更できる。
標準ライブラリの語彙はビルド時の索引に、構文木はプロセス内の固定数の読み取り専用cacheに保持する。
標準ライブラリの cache はユーザーのソース、import別名、型検査結果、policy、所有権状態を保持しない。LSP は別に、リビジョンを区別する上限付き IR cache を持つ。
繰り返しのAPI呼び出しと、新しいプロセスの時間を別々に確認する。

## ネイティブプロファイル

macOSでは対象を1つ指定してRustのスタックを採取できる。出力先は毎回変える。

```sh
just frontend-profile check/std-types target/frontend-profile/check-r1.txt
just frontend-profile compile/workflow target/frontend-profile/compile-r1.txt
```

このタスクはCriterionの `--profile-time` で同じ処理を反復し、macOSの `sample` を1ms間隔で実行する。
Cargoのビルドは採取前に完了させる。プロファイル下の時間は速度比較に使わない。
Linux等ではベンチ実行ファイルに `--bench --profile-time` を渡し、perf等で採取する。
フラグの意味は[Criterionのprofiling資料](https://bheisler.github.io/criterion.rs/book/user_guide/profiling.html)を参照。

## 正しさの検査

`runtime/frontend-reference.test.mjs` は変更前の固定入力、型付きIR、解析結果、WAT、診断をSHA-256等で照合する。
`tests/fixtures/frontend-reference-v4.json` を速度改善のために更新しない。
改名前のv1 referenceも保持し、生成WATが内部namespace名以外は一致することを照合する。
APIが意図的に変わる場合は新しい入力・referenceとして版を分ける。
標準型だけの利用で無関係なライブラリを構文解析しないこと、生成索引とソースの一致、
並行呼び出し時の別名・hygieneの分離もRustのテストで確認する。

## ABI 型グラフの検査

値 ABI のレイアウト、flatten 数、型表現を型 ID ごとにメモ化する。同じ型を二度含む DAG を木として繰り返し展開しない。adapter 内の flatten と WIT の名前付き型生成も結果を再利用する。checked arithmetic と [ABI 上限](wit-bindings.md#abi-型の実装上限) により、許容サイズを超える展開は診断で終了する。

`tests/abi_limits.rs` は重複 DAG と拒否境界を検査し、lib の決定的な訪問数テストは各頂点の再検査を防ぐ。ここでは処理時間の改善率を主張しない。比較する場合は既存の入力と baseline を保ったまま新しい計測ラベルを使う。
