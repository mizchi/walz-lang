# 開発体験の試用 v2

[課題](contract.md)に沿って、CLI・JSONの分類集計・方針変更・子タスク・stdio LSP・native WIT own/borrowを動かす。可読性の所見は[ルーブリック](rubric.md)で別に記録する。実装を知る評価者の試用であり、初見の人間や独立した読解役の評価ではない。

```sh
just eval-usability trial-01
```

Node.js 24以降、Wasm GC対応Wasmtime、インストール済みのpnpm依存が必要。CLIをビルドしてから現在のソースを評価する。生成物は `evals/runs/trial-01/`。IDの再利用・パスの逸脱は拒否する。

- `manifest.json`: 実行CLIのSHA-256、Node版、評価者の立場。
- `inputs.json` / `snapshot/`: 課題・コード・参照資料を固定。WLTのcoreはCLIに同梱されている。
- `commands.json`: CLIの引数・終了コード・stdout/stderr・経過時間。
- `result.json`: 各操作の合否、独立したBigInt/Map oracleとの照合、LSPの実応答、コードの補助指標。
- `work/`: コンパイル結果、生成binding、間違いを入れたコード。

`result.json` の失敗があれば終了コード1。使い勝手の問題を見つける評価なので、正しく動かない操作も記録する。既知の問題を「成功」に変えるため期待値を緩めない。同居した `%bench` を含むアプリのJSビルドも成功することを検査する。以前の問題の最小再現は [bench-repro/main.wlt](bench-repro/main.wlt)。

Core＋JSPIとComponentで同じ7組のデータを比較し、各呼出し後にABIバッファと端点tableの空状態を確認する。方針変更版も7組で検査する。これは任意のGCヒープのリーク不存在の証明ではない。ネイティブWITの検査はraw jco/JSPIを使い、WLT SDKを通さない。今回はブラウザUIや実エディターの操作は測っていない。

JSONのamountは符号付き64bitの整数字句として処理し、小数・指数表記からの丸めは行わない。oracleはJSON.parseのsource contextとBigIntで数値を正確に読み、入力の不正・範囲外・加算overflowを別に検査する。ベンチの実行は機能確認で、速度比較や人間の作業時間の測定ではない。

繰り返す際は新しいIDを使う。課題またはルーブリックを変える場合は版も変える。過去の配布物、試行、診断、報告は書き換えない。コンパイラは複製しないため、厳密な再現にはmanifestと同じCLIとjco依存が必要。

最初の試用結果は [20261108の評価](../reports/20261108-usability-v1/REPORT.md) を参照。

v2は、エラー中のLSP情報とcompletionを検査条件に追加する。JSON課題の意味は同じだが、v1とは点差を比較しない。v1の凍結記録は保持する。

修正後のv2結果は [20261108の修正記録](../reports/20261108-usability-fixes-v2/REPORT.md) を参照。
