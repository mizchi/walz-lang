# 標準名とWITの対応

標準ライブラリの公開型は小文字のkebab-case、関連関数は通常 `$型名.操作名` とする。
JSONの構築・解析・出力・検索は `$json.*` に集約し、可変object writerは `$json.object-builder.*` に分ける。
`$json.from-string` はJSONのstring値の構築、`$json.parse` は文書の解析、`$json.stringify` は値の出力。
WATではドットも識別子の一部で、名前空間を意味しない。WLTも同じで、ファイル単位の公開名を `%use` で明示的に束縛する。
利用者のWAT識別子にはこの命名規則を強制しない。

| 型の公開名 | 関連関数の例 |
| --- | --- |
| `$bytes` | `$bytes.equal`、`$bytes.find` |
| `$string` | `$string.u64`、`$string.find` |
| `$bytes-view` | `$bytes-view.new`、`$bytes-view.slice` |
| `$string-view` | `$string-view.new`、`$string-view.to-string` |
| `$array-view` | `$array-view.new`、`$array-view.slice`、`$array-view.to-list`（要素型と元配列型を具体化） |
| `$dict` | `$dict.new`、`$dict.get` |
| `$list-builder` | `$list-builder.push`、`$list-builder.finish` |
| `$string-builder` | `$string-builder.text`、`$string-builder.finish` |
| `$json-object` | `$json.object-builder.string`、`$json.object-builder.finish` |
| `$json-value` | `$json.parse`、`$json.stringify` |

ビュー、辞書、builder、JSON値の型は不透明なGC型なので、値の型には `(ref $string-view)` や
`(ref (%inst $dict i64))` と書く。`.View` / `.Dict` / `.Builder` の接尾辞は付けない。
公開名の詳細は [生成API一覧](stdlib-api.md) を参照。

## WITの組み込み型とWLT固有の型

| WLTの標準名 | WITでの表現・対応範囲 |
| --- | --- |
| `$string` | `string`。内部は検査済みUTF-8 |
| `$tuple<T,U>` | `tuple<T,U>`。現行実装は2要素のみ |
| `$option<T>` / `$result<T,E>` | `option<T>` / `result<T,E>` |
| `$list<T>` | `list<T>`。要素は共有可能な値に制限 |
| `$future<T>` / `$stream<T>` | `future<T>` / `stream<T>`。既存の非同期adapterで変換 |
| `i32` / `i64` / `f32` / `f64` | 現在のWIT生成は `s32` / `s64` / `f32` / `f64` |
| `$bytes` | WLTの専用表現。WITでは `list<u8>` |
| `$unit` | WLTのunit。結果・payloadの省略や既存unit adapterで表す |
| `$box<T>` | WLTのowned GC box。WITの `own<R>` とは異なる |
| `$resource-handle<R>` / `$borrow<R>` | WLTの外部資源と借用。明示的なhost bindingが必要 |

WITの `own<R>` はresourceの所有権を表すため、GC boxの名前には使わない。
GC boxは `%box.new`、型は `(%inst $box T)`。外部資源は既存の `%resource.*` 操作を使う。
resourceのWIT入出力、一般のn要素tuple、bool/char/各幅のunsigned整数を新しく実装したことは意味しない。
たとえば `$u32` をi32の透明なaliasとして追加しても、符号なし比較・除算の意味を持つ型にはならない。
WATのi32/i64の演算と、WIT境界の型は分けて定義する。

組み込み拡張の語彙も `%string.*`、`%tuple.*`、`%box.*` に揃える。
既存の型付きIR・所有権検査・ABI policyの `text` / `pair` / `own` という内部表現は維持する。
tupleは `%let ($first $second) value body` で分解でき、既存の `case pair` / `case own` の分解ラベルも維持する。

## WITへ出す名前

WLTの `$string-view.slice` は関数の識別子。WITへ出す外部名には `"string-slice"` のような小文字のkebab-caseを指定する。
GC参照はWIT値型ではないため、ビューなどの不透明なGC型は明示的にstring/bytesなどへ変換する。
関連関数をWITのresourceメソッドやconstructorへ自動変換する規則は追加しない。

```wat
(module
  (%use "wlt:core" (type $string)
    (func $string-view.new) (func $string-view.slice) (func $string-view.to-string))
  (func $string-slice (export "string-slice")
    (param $source $string) (param $start i32) (param $length i32)
    (result $string) (%effect allocates)
    (call $string-view.to-string
      (call $string-view.slice (call $string-view.new $source) $start $length))))
```

これはWITで `string-slice: func(p0: string, p1: s32, p2: s32) -> string` になる。
`just wit examples/views.wlt` などで実際の出力を確認できる。

命名は [WITの識別子規則](https://component-model.bytecodealliance.org/design/wit.html#identifiers)
と [WITの型の仕様](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md) に基づく。
WITでは大文字の単語も許されるが、WLT標準ライブラリは小文字に統一する。

## 凍結済みの記録

旧表記は通常のコンパイルでは受理しない。過去の評価・計測ソースは変更せず、既存の内部legacy loaderで読む。
import元の公開名だけを変換し、旧ローカル名・コメント・文字列・元の診断位置を保持する。
現在の評価課題は版を上げ、新しいrunへ入力を固定する。
