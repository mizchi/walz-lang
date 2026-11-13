# Walzソースparser

[English](README.md) | 日本語

`parser.walz` はWalzで書いたUTF-8のS式parserです。デコード済みのtoken bytesと
元ソースのバイト範囲を持つ、opaqueで不変な構文木arenaを返します。
`number.walz` と `string.walz` が文字列とi32/i64リテラルを検査・デコードします。
セルフホストに向けた部品で、Rust版parser全体やWasm validatorの代替ではありません。

[契約・対応構文・上限・検証](../../docs/selfhost.md)と、
[checker](../checker/README.ja.md)・[対応するcodegen](../codegen/README.ja.md)を参照してください。

```wat
(@use "./parser.walz" (func $parser.parse) (type $syntax) (type $diagnostic))
;; $parser.parse(bytes) -> $result<(ref $syntax), (ref $diagnostic)>
```

ホストimportは不要です。構文木は意図的に入力bytesを保持します。
構文木への参照を手放すと、Wasm GCが回収できます。
