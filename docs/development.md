# 処理系の開発

言語を使うためのインストール・最初のプロジェクトは[導入](getting-started.md)を参照。
ここではリポジトリを変更する開発者向けの環境と検査をまとめる。

```sh
git clone https://github.com/mizchi/wlt-lang.git
cd wlt-lang
cargo build --locked --bin wlt
just --list
```

Rust 1.99以降とjustを使う。JavaScript・ブラウザ側はNode.js 24以降、package.jsonで固定したpnpmを使う。
実行テストにはWasm GC対応のWasmtime、WIT検査にはwasm-tools、契約検査にはelan経由のLeanが必要。
処理系が固定しているバージョンとCIの導入方法は[ci/tools.json](../ci/tools.json)と
[インストーラー](../ci/install-tools.py)を参照。native async worldは[専用の実行環境](native-worlds.md)を使う。

```sh
pnpm install --frozen-lockfile
pnpm exec playwright install chromium
```

## 変更に応じた検査

| 対象 | タスク |
| --- | --- |
| Rustの書式・警告・生成API資料 | `just check` |
| コンパイラの回帰 | `just test` |
| WLTの実行テスト | `just test-language` |
| Core＋JSPI / ComponentのJS実行 | `just test-js` |
| ブラウザ実行 | `just test-e2e` |
| 所有権・共有領域の独立カーネル | `just proof-check` |
| WIT契約の往復 | `just wit-check` |
| 導入・言語ガイドの実行例 | `just test-docs` |
| Vite連携 | `just test-vite` |
| Content Mapperの検査とprotocol | `just test-content-mapper` |
| 固定TS7のCLI / LSP / watch | `just test-content-mapper-tsgo`。gitとGoも使用 |
| 固定課題の評価ハーネス | `just test-evals` |

変更対象の検査を先に失敗させ、実装後に成功を確認する。ランタイムの変更は両JS backend、ブラウザ変更はPlaywrightでも検査する。
生成した標準APIは`just stdlib-docs`で更新し、[stdlib-api.md](stdlib-api.md)の署名を手で書き換えない。

## 解析・整形・実行の入口

```sh
cargo run --locked -- check --json examples/result.wlt
cargo run --locked -- analyze examples/result.wlt
cargo run --locked -- ir examples/result.wlt
cargo run --locked -- fmt examples/result.wlt
just samples
just probes
```

サンプルの期待値は[examples/catalog.json](../examples/catalog.json)で管理する。
これと、ガイド本文のコードを抽出して検査する`just test-docs`は別の入口。
[フロントエンド性能](frontend-performance.md)、[wasm-opt](wasm-opt.md)、[アーキテクチャ](architecture.md)も参照。

## 過去の記録

[evals](../evals/README.md)の入力・ルーブリック・結果、docs以下の計測スナップショットは凍結した記録。
再評価は新しいrunとして追加し、当時の数値やコードを書き換えない。
Wasmtime内の試作と旧名gcwatからの移行は[MIGRATION.md](../MIGRATION.md)に保存している。
