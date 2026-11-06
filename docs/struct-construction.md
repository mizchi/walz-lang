# 名前付きstruct構築

`%struct.new`は、既存のGC structをフィールド名で構築する。すべてのフィールドに名前がある型を対象とする。
標準WATの位置引数の`struct.new`も使える。

```wat
(type $User
  (struct (field $name $string) (field $age i32) (field $email $string)))

(%struct.new $User
  ($name (%string.lit "Alice"))
  ($age (i32.const 30))
  ($email (%string.lit "alice@example.com")))
```

`$string`は通常どおり`(%use "wlt:core" (type $string))`で取り込む。
フィールドの記述順は任意。重複、未知のフィールド、欠落を静的エラーにする。nullableやmutableでも省略はできない。
ラベルは変数の束縛ではない。初期値では`$foo`または`(local.get $foo)`で既存のローカル変数を読む。

## 独立した2つの糖衣

フィールド群を`{}`で書ける。`:name`は静的な`$name`フィールドのラベル。

```wat
(%struct.new $User {
  :name (%string.lit "Alice")
  :age (i32.const 30)
  :email (%string.lit "alice@example.com")
})
```

名前付き初期値の直下では、フィールドの型で確定できるリテラルを省略表記できる。`{}`は不要。

```wat
(%struct.new $User
  ($name "Alice")
  ($age 30)
  ($email "alice@example.com"))
```

型名を含む短縮形もある。`%$User::`は`$User`、`%$model.User::`は`$model.User`を指定する。
`%`は拡張構文、`$User`は既存の型識別子を表す。`$`を省いた`%User::`は受け付けない。

```wat
(%$User::{:name "Alice" :age 30 :email "alice@example.com"})
```

裸の数値は、解決したフィールド型の`i32.const` / `i64.const` / `f32.const` / `f64.const`として検査する。
これはフィールド宣言に基づく局所的な期待型付け。例えばi32フィールドの`:age 30`には`(i32.const 30)`と同じ型の定数命令を補い、i64フィールドなら`i64.const`を補う。
リテラルの値から整数幅を決めたり、後続の使用箇所から型を探索したりはしない。型の確定には既存の宣言・命令の署名・明示的な型適用を使う。
明示的に`(i32.const 30)`と書いた値をi64フィールドへ渡すと型エラーになり、暗黙に拡張しない。
範囲・符号・16進数・浮動小数点・NaN/InfはWATの該当型の定数規則に従う。例えばi32フィールドの`4294967295`は全bitが1のi32となる。
裸文字列は`$string`に対する`%string.lit`と同じUTF-8規則を使う。文字列escapeもWATの規則。
数値の既定幅や暗黙変換、newtypeの自動wrap、boolean/nullの別名は追加しない。
bytes、参照、v128、newtype、Option/Resultなどには従来の明示式を使う。
文字列の省略は名前付き構築の初期値の直下に限る。数値とローカル参照は、[期待型が分かる一般の式位置](core-expressions.md#ローカル参照と数値の省略形)でも省略できる。

`{}`は型を指定する構築式の中で使う。型のないmap、spread、comma、動的なキーは追加しない。
値には任意の既存式や、型を指定した別の名前付き構築式を置ける。空structは`(%$Empty::{})`で構築できる。

## 型引数と可視性

既存の名前解決、明示的な型適用、単相化を使う。

```wat
(type $Box (%type.params $T) (%where (shared $T) (nonunit $T))
  (struct (field $value $T)))
(%struct.new (%inst $Box i64) {:value 42})
```

型引数を指定する場合は`%struct.new`の型オペランドに`(%inst ...)`を置く。型引数の推論は追加しない。
%useした型の別名も同じ規則で解決する。opaque型の外部からの構築は認めない。
匿名フィールドを含む型には従来の位置引数構築を使う。

## 評価順と静的解析

値を**ソースの記述順に、到達した式を一度ずつ**評価し、衛生的な一時値へ保存する。
最後に一時値を型定義順へ配置した既存の`struct.new`へ渡す。
途中のthrow・trap・return・分岐では、後続の初期値と最後のstruct割当てを実行しない。
Future待機を含む場合も、先に評価した値を保持する。フィールド名順や型定義順への並べ替えで作用の順番を変えない。

展開結果は既存のtyped IRの`Let`・`Local`・`StructNew`を使い、型・opaque・effect・所有権・メモリ領域の検査を通る。
名前の対応付けはコンパイル時に行い、実行時のkeywordやmapを作らない。structのidentity、allocates、mutableフィールドのget/setはWasm GCの規則を保つ。
共有GCフィールドへの所有値の格納禁止も維持する。所有操作や例外を含む初期値は既存の所有権証明書で検査する。
到達不能な後続式も静的検査の対象に残す。

実行例は[struct-literals.wlt](../examples/struct-literals.wlt)。`just run examples/struct-literals.wlt`は42を返す。
`just test-structs`で型と実行・両JS ABIを、`just test-e2e`でChromiumを検査する。
導入前の読解比較は[評価記録](../evals/reports/20261106-struct-literals-v2/REPORT.md)に保存している。
同じ6パターンを4表記でコンパイル・実行した[現行構文の確認](../evals/reports/20261106-struct-literals-implementation-v2/README.md)も保存している。旧綴り`%Type::`を使った導入時の記録は当時の入力・結果のまま固定している。
