# テーブル処理

[English](README.md) | 日本語

Walzで実装した不変の列指向テーブルと、Arquero風のJS・TS向けAPI。
列選択、絞り込み、列の計算、グループ化、集計、安定ソート、slice、等値結合を扱う。
セルはf64数値、正しいUnicode文字列、bool、null、undefinedに対応する。

- `value.walz`：opaqueなスカラー値、等値比較、順序、グループのキー。
- `engine.walz`：opaqueなテーブル・列・式と、各種変換。
- `number-column.walz`：型のマスクを伴う、必要時に構築するf64x2の数値列キャッシュ。
- `join.walz`：ハッシュ結合、重複行、外部結合、semi／anti join。
- [`runtime/table/`](../../runtime/table/)：JSの式解析とCore向け保持型API。
  同じインスタンス内のGC参照を使い、ハンドル管理表を持たない。
- [`portable.walz`](../../examples/table/portable.walz)：Core＋JSPI・Component向けの
  スナップショットと一括パイプライン実行。

```sh
just build-table
node examples/table/demo.mjs
just test-table
just bench-table benchmarks/table/new-run
```

Walzからは相対パスの`@use`で必要な名前を読み込む。
[ネイティブの例](../../examples/table/main.walz)と
[テーブル処理ガイド](../../docs/table.md)に、API・式・寿命・互換範囲をまとめている。
数値定数との比較と論理合成を、安全に適用できる場合にSIMD化する。
未対応の式にはスカラー処理を使い、比較用のスカラーモードも選べる。
集計はf64の加算順序を保つ。Arqueroの部分的な互換実装であり、
計測方法、キャッシュのメモリ費用、残るAPIはガイドを参照。
