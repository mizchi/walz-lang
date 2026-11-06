# WLT

[English](README.md) | 日本語

Wasm GC と静的解析を前提にした、WAT の小さな拡張言語です。標準 WAT を受け付け、追加構文の入口を `%` で区別します。

- 不変の字句束縛、enum/match、明示的なクロージャ捕捉。
- `wlt:std` からの基本型・標準関数の明示 import と、ジェネリクスの単相化。
- 型・effect・所有権の検査と、Core Wasm / Core＋JSPI / Component への出力。

```wat
(module
  (%import "wlt:std" (type $result))
  (func $main
    (export "main")
    (result i64)
    (%effect pure)
    (%let
      (($a i64 (i64.const 40))
       ($b (i64.const 2))
       ($sum
         (%inst $result i64 i32)
         (%result.ok (i64.add (local.get $a) (local.get $b))))
      )
      (%match (local.get $sum)
        (case ok ($value) (local.get $value))
        (case err ($code) (i64.const 0))))
  ))
```

複数束縛の `%let` は左から右に初期化し、後の初期値から前の束縛を参照できます。この例の `main` は42を返します。

## 試す

Rust 1.97以降、just、Wasm GC 対応の Wasmtime を使用します。

```sh
git clone https://github.com/mizchi/wlt-lang.git
cd wlt-lang
just run examples/closure.wlt     # 42
just wat examples/closure.wlt     # WAT を確認
just test
```

JS 実行には Node.js 24以降と pnpm、検証付き実行には Lean を使います。設定と各コマンドは [導入手順](docs/getting-started.md) にまとめています。

## ドキュメント

| 読みたいこと | 文書 |
| --- | --- |
| 数値命令・Resultの連鎖・newtype・早期脱出 | [Core expressions](docs/core-expressions.md) |
| 文法を一覧する | [チートシート](CHEATSHEET.md)・[仕様](SPEC.md) |
| 標準型、Future、複数束縛 | [標準モジュール](docs/standard.md) |
| 型引数・制約・単相化 | [ジェネリクス](docs/generics.md) |
| 組み込み型クラス・短絡論理・ビット演算 | [型クラス](docs/typeclasses.md) |
| v128・ベクトル演算 | [SIMD](docs/simd.md)・[カーネルと速度比較](docs/simd-kernels.md) |
| 名前付きstruct構築 | [構築と糖衣](docs/struct-construction.md) |
| import/export と生成物の除去 | [静的モジュールと DCE](docs/static-modules.md) |
| effect・型・メモリ領域の解析 | [型検査と静的解析](docs/analysis.md) |
| 所有権と証明 | [検証付き実行](docs/verification.md)・[外部リソース](docs/resources.md) |
| 非同期実行・ABI・取消し | [ランタイム](docs/runtime.md)・[WIT バインディング](docs/wit-bindings.md) |
| テストと初見評価 | [テスト](docs/testing.md)・[evals](evals/README.md) |
| 実装の構成 | [アーキテクチャ](docs/architecture.md) |

[サンプル一覧](examples/catalog.json)、[未対応範囲](ASSESSMENT.md)、[TODO](TODO.md)。
Wasmtime 内の試作からの独立・旧名 gcwat の記録は [MIGRATION.md](MIGRATION.md) に残しています。
