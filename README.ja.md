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
wlt build examples/closure.wlt -o target/closure.wasm
wlt test tests/programs
wlt bench examples/benchmarks/stdlib_v2_bench.wlt
```

Git から直接入れる場合は `cargo install --git https://github.com/mizchi/wlt-lang --locked --bin wlt` を使います。テストには Wasm GC 対応の Wasmtime、ベンチには Node.js 24以降、検証付きビルドには Lean が必要です。既定値とオプションは [CLI ガイド](docs/cli.md)、開発環境は [導入手順](docs/getting-started.md) にまとめています。

[公開プレイグラウンド](https://mizchi.github.io/wlt-lang/) で言語機能を試せます。手元では `just playground` を実行します（wasm-pack が必要）。[プレイグラウンド](docs/playground.md) は、WasmにビルドしたRustコンパイラでローカルにコンパイルします。

## ドキュメント

| 読みたいこと | 文書 |
| --- | --- |
| ブラウザー上で編集・コンパイル・実行 | [プレイグラウンド](docs/playground.md) |
| 数値命令・Resultの連鎖・newtype・早期脱出 | [Core expressions](docs/core-expressions.md) |
| 文法を一覧する | [チートシート](CHEATSHEET.md)・[仕様](SPEC.md) |
| 共有メモリ・atomics・ホストの Worker | [Wasm threads](docs/threads.md)・[atomicセルの検証](docs/atomic-cells.md) |
| 汎用の非同期Workerと不変索引の並列処理 | [Workerホスト](docs/worker-host.md)・[readonly領域](docs/readonly-regions.md)・[簡潔索引](docs/succinct.md) |
| 組み込み型、Future、複数束縛 | [コアモジュール](docs/standard.md) |
| CLI の単体配布、明示したファイル I/O、子タスク | [CLI](docs/cli.md)・[WASI adapter](docs/wasi-files.md)・[spawn/join](docs/scoped-tasks.md) |
| 型付きマクロと WAT メモリ命令の混在 | [validator との橋渡し](docs/wat-validator.md) |
| 型引数・制約・単相化 | [ジェネリクス](docs/generics.md) |
| 組み込み型クラス・短絡論理・ビット演算 | [型クラス](docs/typeclasses.md) |
| v128・ベクトル演算 | [SIMD](docs/simd.md)・[カーネルと速度比較](docs/simd-kernels.md) |
| 名前付きstruct構築 | [構築と糖衣](docs/struct-construction.md) |
| リスト走査・フィールド分解・フィールド更新 | [高水準の糖衣](docs/high-level-forms.md) |
| Option/Result操作・期待型付き文字列 | [値操作と文字列](docs/option-result.md) |
| import/export と生成物の除去 | [静的モジュールと DCE](docs/static-modules.md) |
| ユーザー定義構文と推論した型の利用 | [衛生的マクロ](docs/user-macros.md) |
| Component Wasmの型付きexportを取り込む | [WIT付きWasmの%use](docs/wasm-use.md) |
| effect・型・メモリ領域の解析 | [型検査と静的解析](docs/analysis.md) |
| 所有権と証明 | [検証付き実行](docs/verification.md)・[外部リソース](docs/resources.md) |
| 非同期実行・ABI・取消し | [ランタイム](docs/runtime.md)・[WIT バインディング](docs/wit-bindings.md) |
| テスト・ベンチマークと初見評価 | [テスト](docs/testing.md)・[ベンチマーク](docs/benchmarking.md)・[evals](evals/README.md) |
| Rust側の型検査・コンパイルの計測 | [フロントエンドの性能](docs/frontend-performance.md) |
| 最適化しやすい Wasm GC 出力と Binaryen の計測 | [wasm-opt](docs/wasm-opt.md) |
| エディターと native Component | [LSP](docs/lsp.md) · [Native WIT worlds](docs/native-worlds.md) |
| 実装の構成 | [アーキテクチャ](docs/architecture.md) |

[サンプル一覧](examples/catalog.json)、[初期サンプルの評価](ASSESSMENT.md)、[現在の未対応範囲と TODO](TODO.md)。
Wasmtime 内の試作からの独立・旧名 gcwat の記録は [MIGRATION.md](MIGRATION.md) に残しています。
