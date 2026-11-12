# WALZドキュメント

[English](README.md) | 日本語

WALZを使うための現行ガイド。まず[導入](getting-started.md)、[基本の書き方](language-guide.md)、
[機能一覧](features.md)の順に読む。構文を引く場合は[チートシート](../CHEATSHEET.md)、厳密な規約は[仕様](../SPEC.md)へ進む。
詳しい機能の可否や制限は各ページを参照する。

## 言語を学ぶ

| 読みたいこと | 文書 |
| --- | --- |
| インストール、最初のプロジェクト、test/bench | [導入](getting-started.md)・[CLI](cli.md) |
| 関数、束縛、データ構造、反復、型引数、所有値 | [言語ガイド](language-guide.md) |
| 対応機能と制限を一覧する | [機能一覧](features.md) |
| WATとの境界、数値、早期脱出、Resultの連鎖 | [構文](syntax.md)・[式](core-expressions.md)・[validator](wat-validator.md) |
| コア型とAPIを探す | [walz:core](standard.md)・[生成API一覧](stdlib-api.md)・[命名](naming.md) |
| 型引数、単相化、組み込み演算子 | [ジェネリクス](generics.md)・[型クラス](typeclasses.md) |
| 名前付き構築、list走査、フィールド操作 | [struct](struct-construction.md)・[高水準の糖衣](high-level-forms.md) |
| Option、Result、文字列、list、dict、view | [値操作](option-result.md)・[list](lists.md)・[collections](collections.md)・[views](views.md) |
| JSON、SIMD、索引 | [JSON](json-values.md)・[SIMD](simd.md)・[succinct](succinct.md) |
| FlatBuffersを読み書きする | [FlatBuffersライブラリ](../walz/flatbuffers/README.ja.md) |
| バイト列から整数・浮動小数点を読み書きする | [固定幅・endian 指定アクセス](byte-access.md) |
| サイズ・offset・整列の計算を検査する | [検査付き整数演算](checked-integers.md) |
| 検証済みの境界を関数・ループの最適化に渡す | [不変な範囲の最適化](range-optimization.md) |
| 非公開の tuple・Option・Result の割り当てを除く | [値の最適化](value-optimization.md) |
| ファイルの公開範囲と生成物の除去 | [静的モジュールとDCE](static-modules.md) |
| 構文を定義する | [衛生的な式マクロ](user-macros.md) |

## 実行と外部連携

| 読みたいこと | 文書 |
| --- | --- |
| ブラウザで試す | [Playground](playground.md) |
| ViteからWALZをESM importする | [Vite](vite.md) |
| TS7で宣言ファイルなしに型検査する | [Content Mapper](content-mapper.md) |
| JS ABI、Future/Stream、取消し | [ランタイム](runtime.md)・[batch転送](stream-batches.md) |
| 同期 JS 出力の軽量化を試す | [Direct GC / JS string ABI 実験](js-native-experiment.md) |
| writer、スコープ付き子タスク、期限 | [子タスク](scoped-tasks.md) |
| WITとComponent、型付きWasm依存 | [binding](wit-bindings.md)・[native worlds](native-worlds.md)・[Wasmの@use](wasm-use.md) |
| 明示した標準入出力とファイル | [CLI](cli.md)・[WASI値adapter](wasi-files.md) |
| 共有メモリ・atomics・Worker | [threads](threads.md)・[Workerホスト](worker-host.md) |
| 検証する共有メモリの部分集合 | [atomic cells](atomic-cells.md)・[readonly領域](readonly-regions.md) |

## 検査・計測・開発

| 読みたいこと | 文書 |
| --- | --- |
| 型、effect、メモリ解析 | [静的解析](analysis.md)・[JSON診断と型問い合わせ](tooling.md) |
| GCの到達性、終了処理、反復後のメモリ保持 | [メモリ管理](../MEMORY.md)・[2026-11-12の検査](memory/20261112-audit-r2/README.md) |
| 所有権、外部資源、law、証明の範囲 | [検証](verification.md)・[resource](resources.md)・[独立カーネル](../proof/README.md) |
| エディターとエラー | [LSP](lsp.md)・[診断](diagnostics.md) |
| 言語内のtestとbench | [テスト](testing.md)・[ベンチマーク](benchmarking.md) |
| 生成コードとコンパイラの性能 | [wasm-opt](wasm-opt.md)・[SIMD計測](simd-kernels.md)・[Criterion](frontend-performance.md) |
| 処理系を変更する、検査を再現する | [開発ガイド](development.md)・[アーキテクチャ](architecture.md) |

## サンプルと記録

小さい例は[サンプルカタログ](../examples/catalog.json)から探す。
一連の使い方は[CLI](../examples/cli/README.md)・[Vite](../examples/vite/README.md)、
複雑な処理は[ログ集計](../examples/apps/log-stats/README.md)・[在庫台帳](../examples/apps/stock-ledger/README.md)・
[ワークフロー](../examples/apps/workflow/README.md)を参照。

[TODO](../TODO.md)に残る実装課題、[MIGRATION](../MIGRATION.md)に旧名gcwatとWasmtimeからの独立を記録する。
[evals](../evals/README.md)、grammar-eval、readability、日付付きの計測資料は当時の入力・結果を保存した記録。
現行の構文やAPIは、この索引からたどるガイド・仕様・生成APIを参照する。
