# Walz

[English](README.md) | 日本語

Walz は、JavaScript・TypeScriptから使う検索・集計・バイナリ処理エンジンなど、データ処理ライブラリをWebAssemblyで実装するための、WATのスーパーセット言語です。

WATのS式構文とWasmの意味論を維持し、Wasm GCを前提にジェネリクスと高水準の操作を追加します。GCによる構造化データと明示的なSIMD命令を同じプログラムで扱い、型付き拡張コアでは型・effect・所有権を検査します。

実装例には[Arquero風のテーブル処理](docs/table.md)、[列データの絞り込みと集計](docs/dictionary-column.md)、[bitmapと数値演算](docs/simd-collections.md)、[簡潔データ構造の索引](docs/succinct.md)、[JSON処理](docs/json-values.md)があります。データを計算内で保持し、必要に応じて処理をまとめます。[辞書列の検索](benchmarks/dictionary-column/README.md)と[テーブルの計測](benchmarks/table/README.md)では、保持済みデータの処理と、構築・ABI変換・入出力全体の費用を分けています。

データ構造の抽象は、[WASI 0.3](https://github.com/WebAssembly/WASI/releases/tag/v0.3.0) の `result`・`option`・`stream`・`future` を想定し、WIT や Component Model との受け渡しがしやすい形にそろえています。これらの型は `walz:core` から明示的に import します。現在の ABI とホスト連携の対応範囲は [ランタイム](docs/runtime.md) を参照してください。

`walz:core` は組み込みのデータ型とポータブルなデータ操作を提供します。OSの機能やI/O capabilityは、明示的なホストimportを通じて受け取ります。

- `@` で区別する拡張構文。不変の束縛、ループ、enum/match、名前付きstruct構築、明示的なクロージャ捕捉。
- 制約付きジェネリクスの単相化。一意に決まる関数の型引数は省略可能。
- 束縛スコープを明示し、通常の型検査器へ型を問い合わせられる衛生的な式マクロ。
- 型付き拡張コアでの型・effect・所有権の検査。
- Core Wasm / Core＋JSPI / Component への出力。
- CLI、ブラウザPlayground、LSP、ViteのESM import、TS7 Content Mapper。

```wat
(module
  (@use "walz:core" (type $result))
  (func $add (@type.params $T) (@where (Add $T))
    (param $a $T) (param $b $T) (result $T) (@effect pure)
    (+ $a $b))

  (func $main (export "main") (result i64) (@effect pure)
    (@let $answer (@inst $result i64 i32)
      (@result.ok (call $add 40 2))
      (@match $answer
        (@case ok ($value) $value)
        (@case err ($code) 0)))))
```

`@inst` は型引数の具体化です。通常の汎用関数の呼び出しでは、引数と期待する結果から型引数を推論できます。`@let` で不変の値を束縛し、`@match` で Result の両ケースを処理します。この例の `main` は42を返します。

ソースも印字も、基本形式はWalzのS式です。`walz print file.walz` はWalzを出力し、`walz print --format watz file.walz` で人間のレビュー向けの [Watz表示](docs/review-syntax.md) を選べます。

## 試す

Rust 1.99以降の Cargo で CLI をインストールします。

```sh
cargo install --git https://github.com/mizchi/walz-lang --locked --bin walz
walz new app
cd app
walz run                    # 42
walz build                  # target/main.wasm
walz test
walz bench --list
walz bench --samples 10
```

checkoutからは`cargo install --path . --locked --bin walz`でインストールします。`walz new`はオフラインでソース・テスト・ベンチ・英日資料と[Walz skill](docs/agent-skills.md)を生成します。テストには Wasm GC 対応の Wasmtime、ベンチには Node.js 24以降、law・race-free契約・検証付き実行にはLeanが必要です。既定値とオプションは[CLIガイド](docs/cli.md)、最初のプロジェクトの作り方は[導入手順](docs/getting-started.md)にまとめています。

[公開プレイグラウンド](https://mizchi.github.io/walz-lang/) で言語機能を試せます。手元では `just playground` を実行します（wasm-pack が必要）。[プレイグラウンド](docs/playground.md) は、WasmにビルドしたRustコンパイラでローカルにコンパイルします。

## ドキュメント

現行のガイドは英語を基本とし、README・チートシート・仕様には日本語版も用意しています。過去の評価記録は元の言語で保存します。
最初は導入手順、次に言語ガイドへ進んでください。機能一覧には実装済みの機能と保証の範囲をまとめています。

| 読みたいこと | 文書 |
| --- | --- |
| インストールし、最初のプログラムを実行・テスト・計測 | [導入](docs/getting-started.md)・[CLI](docs/cli.md) |
| 実行できるコードで基本の書き方を学ぶ | [言語ガイド](docs/language-guide.md) |
| 機能・対応範囲・制限を一覧する | [機能一覧](docs/features.md) |
| 目的に合う詳細ガイドを探す | [ドキュメント索引](docs/README.ja.md) |
| 構文と契約を引く | [チートシート](CHEATSHEET.ja.md) / [English](CHEATSHEET.md)・[仕様](SPEC.ja.md) / [English](SPEC.md)・[コアAPI](docs/stdlib-api.md) |
| JavaScript・TypeScriptから使う | [Vite ESM](docs/vite.md)・[TS7 Content Mapper](docs/content-mapper.md) |
| コーディングエージェントでWALZを読み書きする | [skillとCLI例](docs/agent-skills.md) |
| 処理系を変更する | [開発](docs/development.md)・[アーキテクチャ](docs/architecture.md) |

[サンプル一覧](examples/catalog.json)、[初期サンプルの評価](ASSESSMENT.md)、[現在の未対応範囲と TODO](TODO.md)。
Wasmtime 内の試作からの独立・旧名 gcwat の記録は [MIGRATION.md](MIGRATION.md) に残しています。

## ライセンス

[Apache License 2.0](LICENSE)。
