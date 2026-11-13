# SIMDコレクション

[English](README.md) | 日本語

繰り返すバルククエリ向けの、不変なWasm GCコレクション。

- `bitmap.walz`：128bit単位の集合演算、個数・積集合の集計、選択行IDの列挙。
- `i32-column.walz`：4つのi32を単位とする列、等値・範囲選択、bitmapを作らない個数集計、正確なi64合計、INと範囲を融合した行クエリ。
- `dictionary-column.walz`：不変の文字列辞書と検証済みi32コード、値による選択・復号・一括集計、数値条件と融合したクエリ。

相対パスの`@use`で必要な型・関数だけを読み込む。
言語構文・ホスト依存・ランタイムの管理表は追加していない。
`walz:core`のファサードとは別のライブラリ。

[契約とサンプル](../../docs/simd-collections.md)、
[辞書符号化列](../../docs/dictionary-column.md)、
[クエリ全体](../../examples/simd-collections/main.walz)、
[JS・Component向けの値変換](../../examples/simd-collections/portable.walz)を参照。

```sh
walz run examples/simd-collections/main.walz
just test-simd-collections
just bench-simd-collections target/simd-collections-bench/new-run
just test-dictionary-column
just bench-dictionary-column target/dictionary-bench/new-run
```

構築時にデータをコピーする。複数回のクエリでは構築済みの値を保持する。
JSを置き換える判断ではABI変換の時間も測る。
内部の`v128`はWIT・JSのABIには渡さず、GC参照をWorker間で共有しない。
辞書列を保持した走査はJSのコード走査より速い条件が確認できた。
JSの事前集計・行ID索引や、文字列を毎回ABI転送する処理では結果が異なる。
[辞書列の測定結果](../../docs/dictionary-column.md#measured-performance)を参照。
