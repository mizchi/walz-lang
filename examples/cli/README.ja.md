# CLI の例

[English](README.md) | 日本語

リポジトリのルートから `cargo install --path . --locked --bin walz` でインストールします。
その後、このディレクトリで実行します。

```sh
walz build                  # target/main.wasm
walz test                   # 1 passed
walz bench                  # measures the answer function
```

テストには Wasm GC に対応した Wasmtime、ベンチマークには Node.js 24 以降が必要です。
この例では静的ファイルモジュールを共有し、`walz:core` から `option` を import します。
ベンチマークは定数入力で CLI の動作を確認するためのもので、計測時間は性能比較には適しません。
