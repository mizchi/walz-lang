# WLT

[English](README.md) | 日本語

WLT は、S式の WAT（WebAssembly Text Format）のスーパーセットを維持しながら、Wasm GC を前提にジェネリクスや高水準の命令を追加したプログラミング言語です。Wasm の意味論を保ちつつ、WAT をより書きやすくすることを目指しています。

データ構造の抽象は、[WASI 0.3](https://github.com/WebAssembly/WASI/releases/tag/v0.3.0) の `result`・`option`・`stream`・`future` を想定し、WIT や Component Model との受け渡しがしやすい形にそろえています。これらの型は `wlt:core` から明示的に import します。現在の ABI とホスト連携の対応範囲は [ランタイム](docs/runtime.md) を参照してください。

`wlt:core` は組み込みのデータ型とポータブルなデータ操作を提供します。OSの機能やI/O capabilityは、明示的なホストimportを通じて受け取ります。

- `%` で区別する拡張構文。不変の束縛、ループ、enum/match、名前付きstruct構築、明示的なクロージャ捕捉。
- 制約付きジェネリクスの単相化。一意に決まる関数の型引数は省略可能。
- 束縛スコープを明示し、通常の型検査器へ型を問い合わせられる衛生的な式マクロ。
- 型付き拡張コアでの型・effect・所有権の検査。
- Core Wasm / Core＋JSPI / Component への出力。
- CLI、ブラウザPlayground、LSP、ViteのESM import、TS7 Content Mapper。

```wat
(module
  (%use "wlt:core" (type $result))
  (func $add (%type.params $T) (%where (Add $T))
    (param $a $T) (param $b $T) (result $T) (%effect pure)
    (+ $a $b))

  (func $main (export "main") (result i64) (%effect pure)
    (%let $answer (%inst $result i64 i32)
      (%result.ok (call $add 40 2))
      (%match $answer
        (case ok ($value) $value)
        (case err ($code) 0)))))
```

`%inst` は型引数の具体化です。通常の汎用関数の呼び出しでは、引数と期待する結果から型引数を推論できます。`%let` で不変の値を束縛し、`%match` で Result の両ケースを処理します。この例の `main` は42を返します。

## 試す

Rust 1.99以降の Cargo で CLI をインストールします。

```sh
git clone https://github.com/mizchi/wlt-lang.git
cd wlt-lang
cargo install --path . --locked --bin wlt
wlt run examples/closure.wlt              # 42
wlt build examples/closure.wlt -o target/closure.wasm
wlt test examples/cli/tests
wlt bench examples/cli/answer_bench.wlt
```

Git から直接入れる場合は `cargo install --git https://github.com/mizchi/wlt-lang --locked --bin wlt` を使います。テストには Wasm GC 対応の Wasmtime、ベンチには Node.js 24以降、law・race-free契約・検証付き実行にはLeanが必要です。既定値とオプションは[CLIガイド](docs/cli.md)、最初のプロジェクトの作り方は[導入手順](docs/getting-started.md)にまとめています。

[公開プレイグラウンド](https://mizchi.github.io/wlt-lang/) で言語機能を試せます。手元では `just playground` を実行します（wasm-pack が必要）。[プレイグラウンド](docs/playground.md) は、WasmにビルドしたRustコンパイラでローカルにコンパイルします。

## ドキュメント

最初は導入手順、次に言語ガイドへ進んでください。機能一覧には実装済みの機能と保証の範囲をまとめています。

| 読みたいこと | 文書 |
| --- | --- |
| インストールし、最初のプログラムを実行・テスト・計測 | [導入](docs/getting-started.md)・[CLI](docs/cli.md) |
| 実行できるコードで基本の書き方を学ぶ | [言語ガイド](docs/language-guide.md) |
| 機能・対応範囲・制限を一覧する | [機能一覧](docs/features.md) |
| 目的に合う詳細ガイドを探す | [ドキュメント索引](docs/README.md) |
| 構文と契約を引く | [チートシート](CHEATSHEET.md)・[仕様](SPEC.md)・[コアAPI](docs/stdlib-api.md) |
| JavaScript・TypeScriptから使う | [Vite ESM](docs/vite.md)・[TS7 Content Mapper](docs/content-mapper.md) |
| 処理系を変更する | [開発](docs/development.md)・[アーキテクチャ](docs/architecture.md) |

[サンプル一覧](examples/catalog.json)、[初期サンプルの評価](ASSESSMENT.md)、[現在の未対応範囲と TODO](TODO.md)。
Wasmtime 内の試作からの独立・旧名 gcwat の記録は [MIGRATION.md](MIGRATION.md) に残しています。
