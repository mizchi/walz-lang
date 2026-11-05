# WLT への移行

このリポジトリは Wasmtime の作業ディレクトリにあった `examples/gcwat` の試作を取り出したものです。
Wasmtime 自体の Git 履歴やソースは取り込んでいません。最初のコミットに改名前の試作を保存し、その後で改名しています。
ライセンスは元の Cargo 宣言に従い Apache-2.0 WITH LLVM-exception を維持します。

| 対象 | 旧名 | 現行名 |
| --- | --- | --- |
| 言語・リポジトリ | gcwat / examples/gcwat | WLT / mizchi/wlt-lang |
| Rust crate・実行ファイル | gcwat | wlt |
| 拡張ソース・探索対象のテスト | .gwat / .test.gwat | .wlt / .test.wlt |
| WIT namespace | gcwat:* | wlt:* |
| private Core ABI marker | gcwat-core-js-v2 | wlt-core-js-v2 |
| JS の runtime package | gcwat-runtime | @mizchi/wlt-runtime |
| backend 選択・stress 検査 | GCWAT_BACKEND / GCWAT_WORKFLOW_STRESS | WLT_BACKEND / WLT_WORKFLOW_STRESS |

CLI は `cargo install --locked --path .` でインストールし、`wlt check file.wlt`、`wlt run file.wlt`、`wlt test directory` として使えます。
`just` の既定パスとサンプルも `.wlt` に変更しました。整形タスク名は `fmt-wlt` / `check-wlt-format` です。
標準 WAT の `.wat` は変更しません。入力ファイルの拡張子を強制する変更もしていません。

WIT import と private ABI の名前も変更したため、旧名で生成した Component / Core＋JSPI の成果物は再ビルドしてください。
既存の外部コードは import 名・環境変数・生成コードの設定を現行名に合わせます。

`docs/grammar-eval/`、`docs/readability/`、`evals/reports/` と workflow の `validation.json`・`tuning.json`・評価／計測レポートは過去の記録です。
入力とハッシュの対応を保つため、旧名・旧拡張子・当時のパスを残しています。これらは現行版を再評価した結果ではありません。
当時の入力を再実行する場合は改名前の初期コミットを別の checkout に取り出してください。
現行の評価 suite は版を 2 に上げており、`just eval-prepare ID` / `just eval-diagnostics-prepare ID` で新しい入力を固定できます。

`target/`、`node_modules/`、`evals/runs/`、ブラウザの検査出力は公開するソースに含めません。
既存のローカル成果物に記録された旧ディレクトリは書き換えず、新しい計測は新しい capture/run に保存します。

Rust 1.97 以降、just、Node.js 24 以降、pnpm を使います。実行検査には Wasmtime CLI、WIT 検査には wasm-tools、証明検査には elan と Lean v4.34.1 が必要です。
Wasmtime の checkout への path dependency はありません。依存は Cargo.lock と pnpm-lock.yaml に固定しています。

## 移行時の検査

2026-11-05、独立した `wlt-lang` ディレクトリで次を確認しました。

- `just check`：整形・全 target の clippy。
- `just test`：Rust の198件。
- `just test-language`：`.test.wlt` の32件。
- `node --expose-gc --test --test-force-exit runtime/*.test.mjs`：43件。Core / Component の取消しと GC 到達性を含む。
- `just test-e2e`：Chromium の24件。
- `just test-evals`：評価ハーネスの11件。
- `just proof-check` と `just wit-check`：Lean の4モデルと WIT の往復検査。
- `pnpm install --frozen-lockfile`：固定した依存のインストール。
- `just build-workflow` → `just workflow`：JSON Lines をパースし、全レコードを `expected.json` と照合。

CLI の回帰検査は一時ディレクトリから `wlt test .` を実行し、改名後のテスト探索が checkout の場所に依存しないことも確かめます。
