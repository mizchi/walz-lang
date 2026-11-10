# walz 文法チートシート

[English](CHEATSHEET.md) | [日本語](CHEATSHEET.ja.md)

標準 WAT を基底に、型付きの式・GC データ・所有値・衛生的マクロを追加した言語。
ここでは拡張を使う型付きコアを扱う。Walz の拡張入口には `@` を付ける。標準 WAT 単体はそのまま受理し、コア外の WAT と構文マクロの混在は opaque として全体を検証する。[構文の境界](docs/syntax.md) を参照。
詳しい意味論は [SPEC.md](SPEC.ja.md)、実用例は [在庫台帳](examples/apps/stock-ledger/README.md)。

## 最初に覚える規則

- 式は `(操作 引数...)`。式位置の `$foo` は `(local.get $foo)`。数値は期待型が分かれば `42`、明示するなら `(i64.const 42)`。文字列は `(@string.lit "hello")`、期待型が `$string` なら `"hello"` と書く。
- プログラムは `(module 宣言...)`。関数の本体は1つの式。複数の処理には block を使う。
- `$name` は識別子、`"text"` は文字列、`;;` は行コメント、`(; ... ;)` は入れ子可能なコメント。
- 引数と演算は左から右へ評価する。暗黙の数値変換・捕捉・ローカル再代入はない。
- **`(result R)` は結果型の宣言。`(@inst $result T E)` は成功 T／エラー E の値型。**
- 結果宣言を省略すると unit。本体から結果型を推論する指定ではない。
- 共有 GC 参照と、消費が必要な own / Future / Stream を区別する。

以下の `T U E S`、`expr`、`body`、`...`、`[省略可能]` は説明用の記号。字面をソースに書くものではない。
`wat` のコードブロックはそれぞれコンパイルできる完全なモジュール、`text` は構文のひな形。

## 標準 WAT と `@` 拡張の境界

| 入口 | 意味 |
|---|---|
| `func`・`param`・`result`・`if`・`block`・`call` | 標準 WAT の関数・型宣言・式。`if` に `@` は付けない |
| `@let`・`@for`・`@for-each`・`@loop`・`@recur`・`@match`・`@try` | Walz の束縛・制御構文。標準の `loop` / `try` と区別する |
| `(result i64)` / `(result i32 i64)` | 標準 WAT の結果型宣言。後者は標準 WAT の複数戻り値 |
| `(@inst $result T E)` | 成功 T／エラー E を持つ Walz の値型。返す関数は `(result (@inst $result T E))` |
| `(import "host" "member" ...)` / `(export "name" ...)` | Wasm のインスタンス化時の import／生成モジュールの export |
| `(@use "./file.walz" ...)` / `(@export ...)` | Walz ソースの選択的な静的リンク／ソース API の公開 |
| `(@use "walz:core" ...)` | コンパイラに同梱した標準型・標準関数を名前ごとに取り込む |
| `@closure`・`@capture`・`@effect`・`@type.params`・`@where` | Walz が追加した宣言・注釈 |
| `@macro` / `@$name` | 衛生的な式マクロの宣言／呼び出し。標準 WAT の命令は再定義しない |

`@` は拡張の入口に付ける。内部の `case`、`bind`、`while`、`step`、`do`、`catch`、制約の `shared` / `nonunit` / `callable` は、それぞれの入口に属する構文なので接頭辞を付けない。識別子は従来どおり `$name`。

既知の Walz `@` 名は予約され、必ず検査する。それ以外の WAT 注釈は、`@custom` やその入れ子の内容も含めて opaque として扱う。

標準 WAT 単体はそのまま検証・コンパイルする。以下の型付きコアでは型・effect・所有権を検査する。
コア外の WAT 命令と混在できる拡張は、構文だけで展開できる `@let` / `@loop` / `@for` などに限る。その経路は Wasm 全体を検証し、`analysis.opaque=true` と保守的な effect を返す。`@effect pure` を付けて詳細な解析や所有権の保証を得ることはできず、検査を必要とする拡張との併用は受理しない。
型付きコアの戻り値は0個（unit）または1値。pair も1値として返す。標準 WAT の `(result i32 i64)` は標準 WAT 経路で扱い、型付きコアの pair 分解へは変換しない。

## ユーザー定義マクロ

`@macro` は衛生的な式マクロを宣言し、`@$name` で呼ぶ。`?name:expr/ident/type/literal` で構文を捕捉し、末尾の `...` で1段の繰り返しを表す。展開後も通常の型・effect・所有権検査を通る。[詳しい規約](docs/user-macros.md)。

```wat
(module
  (@macro $twice
    (rule (?value:expr)
      (infer (?T ?value))
      (when (Add ?value))
      (@let $saved (type ?T) ?value (+ $saved $saved))))
  (@macro $with
    (rule (?name:ident ?value:expr ?body:expr)
      (bind ?name ?body)
      (@let ?name ?value ?body)))
  (func $main (export "main") (result i32)
    (@effect pure)
    (@$with $x (i32.const 21) (@$twice $x)))
  (@test "explicit binding and inferred arithmetic" (= (call $main) 42)))
```

`infer` は通常の型推論から具体的な型の構文を取得する。`(infer (?T expected))` は呼び出しの期待型、`(infer (?T ?value expected))` はその期待型を明示的なヒントにして引数を検査する。整数幅を値から推測しない。`when` は具体型 `(type ?value i32)` や既存の型クラスでルールを選ぶ。`bind` は呼び出し側の名前が有効になる本体を宣言する。

`(@export (macro $twice))` と `(@use "./syntax.walz" (macro $twice $double))` で選択・別名・再公開ができる。`walz macro-expand file.walz` は構造化した型情報と展開元を含むJSON、`--text` は入力の single / duplicated / discarded と構文出現数を表示する。実行回数は制御フローによる。`walz expand` は従来どおり最終WATを出力する。

`(infer (?T (option.element ?value)))` のように型構造を問い合わせられる。
selector は `option.element` / `list.element` / `result.ok` / `result.err` / `tuple.first` / `tuple.second` /
`box.element` / `future.element` / `stream.element` / `task.element` / `array.element` / `ref.heap`。
実行はせず、opaque な配列の内部型は外部から読めない。[型情報・差分fuzzと再現手順](docs/user-macros.md)。

`bind` の ident は反復も使える。テンプレートの let グループは反復を展開してから下げる。

```text
(@macro $bindings
  (rule (((?name:ident ?value:expr) ...) ?body:expr)
    (bind ?name ?body)
    (@let ((?name ?value) ...) ?body)))
(@$bindings (($a (i32.const 20)) ($b (i32.const 22))) (+ $a $b))
```

呼び出し先と同じ `bind` を宣言すると束縛を委譲できる。新しい名前が見えるのは指定した本体だけで、
初期値は caller のスコープ。空列は本体を評価し、同名の重複は拒否する。
本体を委譲するときは `?body` を丸ごと渡す。`(block ... ?body)` のように組み直したスコープは推定しない。
マクロの初期値で先に生成した名前を参照したい場合は、依存する値を通常の `@let` で先に束縛して渡す。

`walz macro-expand --text examples/user-macros/main.walz` で展開・型・入力の利用状況を確認できる。
JSON の `checking` は全体の検査パス数、関数本体へ入った回数、予算で数える構文走査を表し、実行時間や全検査操作数ではない。
通常は展開した式から検査を再開し、具体化で型環境が変わる場合は全体を検査し直す。
[多段の実行例](examples/user-macros/bindings.walz)。

## `walz:core` と基本データ型

Walz の基本型は `walz:core` から明示的に import する。`i32` などの Wasm の型はそのまま使う。

```text
(@use "walz:core"
  (type $result) (type $option) (type $future) (type $stream)
  (type $tuple) (type $list) (type $box)
  (type $string) (type $bytes) (type $unit)
  (func $string.u64 $format))

(result (@inst $result $string i32))
(param $pending (@inst $future (@inst $result $string i32)))
(call $format (i64.const 42))
```

型も汎用関数も `(@inst $name T...)` で適用する。通常の `call $generic` は引数・期待型から一意に決まる型引数を省略できる。`$string` / `$bytes` / `$unit` は型引数なしの alias なので、その名前で参照する。`(type $result $Outcome)` のような別名も使える。
名前は import したスコープにだけ束縛される。暗黙の prelude はなく、別ファイル で使う名前もそこで import する。標準関数は `$string.*`、`$bytes.*`、`$list.map` / `$list.fold`、`$string-builder.*`、`$json.*` から選ぶ。`walz:core` はファイルパスやホスト import ではなく、通常のライブラリの静的リンクと DCE を使う。
`@string.lit`・`@result.ok`・`@future.await` などは値を操作するコンパイラの固定構文。型の参照には import した名前と `@inst` を使う。
関数の完全な型・型制約・effect は、公開宣言から生成する [標準 API 一覧](docs/stdlib-api.md) を参照する。
`$dict.*` はtextキーの汎用辞書、`$list-builder.*` は不変listの構築。どちらも共有可能・非unitの要素を明示的に型適用し、内部表現はopaqueにする。[コレクション](docs/collections.md) を参照。

```wat
(module
  (@use "walz:core" (type $future))
  (func $main
    (export "main")
    (result i64)
    (@effect suspends)
    (@let
      (($pending (type (@inst $future i64)) (@future.ready i64 40))
       ($base (@future.await i64 (@move $pending)))
       ($answer (type i64) (i64.add $base 2)))
      $answer))
  (@test "standard Future" (i64.eq (call $main) (i64.const 42))))
```

## ゼロコピービュー

標準型は小文字のkebab-case、関連関数は`$型名.操作名`。`$tuple`はWITの2要素tuple、`$box`はWITの`own<resource>`とは異なるowned GC box。[WITとの対応](docs/naming.md)。

標準文字列型は`$string`。`walz:core`の`$bytes-view` / `$string-view`は不透明なGC型で、`(ref $string-view)`のように参照する。
`new`で全体、`slice`で相対的な`start,length`のビューを作る。データ本体は共有し、ビュー自身だけをGCに確保するため`allocates`を持つ。
stringの範囲はUTF-8のバイト単位で、文字途中の境界と範囲外はtrap。`bytes`で同じ範囲のbytesビューを得られる。
`to-bytes` / `to-string`は表示範囲をコピーし、元のバッファを切り離す。WITの境界にはこの変換後の値を渡す。
詳細と実行例は[ビュー](docs/views.md)、[examples/views.walz](examples/views.walz)。

GC配列には`(ref (@inst $array-view T $Array))`を使う。要素型Tと名前付き配列型を明示し、`walz:core`から関数をimportする。
`(call (@inst $array-view.new i64 $Numbers) $source)`で全体、`slice`で要素単位の相対範囲を共有し、`get`で読む。
ビュー自体は読み取り専用だが、元の可変配列への変更は反映される。`get`はreads、`new`/`slice`はallocatesを持つ。
`to-list`で表示範囲を不変listへ明示的にコピーできる。詳しい制約・GC保持・ABIの扱いは[ビュー](docs/views.md#任意のgc配列を共有するarray-view)を参照。

## 関数・型・束縛

```wat
(module
  (func $add
    (param $a i64)
    (param $b i64)
    (result i64)
    (@effect pure)
    (i64.add (local.get $a) (local.get $b)))

  (func $main
    (export "main")
    (result i64)
    (@effect pure)
    (@let $answer (call $add (i64.const 40) (i64.const 2))
      (local.get $answer)))
  (@test "answer" (i64.eq (call $main) (i64.const 42))))
```

| 型 | 意味 |
|---|---|
| `i32 i64 f32 f64` | Wasm の数値型。整数の加減乗算は折り返し、整数のゼロ除算等は trap。符号解釈は命令の `_s` / `_u` で指定 |
| `v128` | Wasm の128ビット値。`v128.const`、splat、lane操作、算術・比較・shuffle等は標準命令名を使う。[SIMD](docs/simd.md) |
| `(ref $T)` / `(ref null $T)` | GC の非 null／nullable 参照。型は nominal |
| `$bytes` / `$string` | 不変なバイト列／妥当な UTF-8。内部参照の操作や比較はできない |
| `(@inst $list T)` | 共有値を要素とする不変リスト |
| `(@inst $option T)` / `(@inst $result T E)` | none/some、ok/err の値型 |
| `(@inst $tuple T U)` | 2成分の値。成分が共有値なら pair も共有値 |
| `(@inst $box T)` / `(@inst $future T)` / `(@inst $stream T)` | 消費する所有値。GC 参照への排他的所有を意味しない |
| `$unit` | 値のない結果。関数の引数・通常のフィールドには格納しない |

```text
(func $f (param $x T)... [(result R)] [(@effect ...)] body)
(@let $x [(type T)] initializer body) ;; 注釈省略時は初期値の型。不変の字句束縛
(@let (($a [(type T)] initial-a) ($b [(type U)] initial-b) ...) body)
(local.get $x)                      ;; 式位置では $x とも書ける
(call $f arg...)
(block [(result R)] unit-expr... final-expr)
(if [(result R)] i32-condition (then expr) (else expr))
(drop shared-value)                 ;; unit の式へ。所有値は drop しない
(nop)                              ;; unit
(unreachable)                      ;; trap。Wasm 例外とは異なる
```

if / block / match の結果宣言がない場合も unit。unit の戻り値は `(result $unit)` と書かず、結果宣言を省略する。
最後以外の block の式は unit にする。
一時的な整数を捨てる場合は `(drop ...)`、状態の更新は mutable な GC フィールド／配列へ明示する。

複数束縛の `@let` は、初期値を左から右に一度ずつ評価する。後の初期値と body からは前の束縛を参照でき、後の束縛の先行参照はできない。同名の再束縛は、その初期値の評価後から以前の名前を隠す。個々の型注釈は省略できる。これはネストした `@let` と同じ意味であり、Wasm の複数戻り値を分解する指定ではない。
pair の分解には既存の `(@let ($a $b) subject body)` を使う。どちらの形式も型・effect・所有権を検査し、所有値には明示的な move と消費が必要。

```text
(@let {:category $key :amount $amount} $row
  (call $add $key $amount))

(@for-each ($key $amount) $entries
  (call $add $key $amount))

(@struct.update $State $state
  ($accepted $old (+ $old 1)))
```

フィールド分解はnon-nullの共有構造体から必要なフィールドだけ選ぶ。対象を一度評価し、値を記述順に読んで本体に束縛する。`@for-each` は共有可能な非unit要素のlistを順に走査し、全体・本体ともunit。単一名・tuple・フィールドpatternを使え、脱出には外側のblock/brを使う。`@struct.update` は可変フィールドを一度読み、旧値を使って計算した値を一度書く。unitを返し、原子性は持たない。[規約と実行例](docs/high-level-forms.md)。

```wat
(module
  (func $main (export "main") (result i32)
    (@let $foo (type i32) 41
      (i32.add $foo 1))))
```

`1` の型は演算命令・引数・戻り値・フィールド・`(type T)`注釈などから決める。既定の整数幅は設けず、`(@let $x 1 ...)` と `(drop 1)` は曖昧なのでエラー。`$foo`は既存のローカルを読むだけで、暗黙のmoveや変換はしない。[省略式の規約](docs/core-expressions.md#ローカル参照と数値の省略形)。

## 基本型の型クラス

| クラス | 対応型 | 2引数の演算子 |
| --- | --- | --- |
| `Add` | i32 / i64 / f32 / f64 | `+` |
| `Sub` | i32 / i64 / f32 / f64 | `-` |
| `Mul` | i32 / i64 / f32 / f64 | `*` |
| `Div` | i32 / i64 / f32 / f64 | `/`。整数は符号付き除算 |
| `Eq` | i32 / i64 | `=` / `!=` |
| `Ord` | i32 / i64 | `<` / `<=` / `>` / `>=`。符号付きの比較 |
| `Logical` | i32 | `&&` / `\|\|`。短絡評価して0/1を返す |
| `Bitwise` | i32 / i64 | `&` / `\|` / `^`。両辺を評価するビット演算 |
| `Shift` | i32 / i64 | `<<` / `>>` / `>>>`。左・符号付き右・ゼロ埋め右シフト |

```wat
(module
  (func $four (result i32) (+ 1 3))
  (func $add (@type.params $T) (@where (Add $T))
    (param $a $T) (param $b $T) (result $T) (@effect pure)
    (+ $a $b))
  (func $main (export "main") (result i64)
    (call (@inst $add i64) 40 2))
  (@test "literal plus" (= (call $four) 4))
  (@test "short circuit" (|| (= (call $four) 4) (unreachable)))
  (@test "masked shift" (= (i64.const 1) (<< (i64.const 1) 64))))
```

固定の記号演算子には`@`を付けない。`+`は期待型または型の分かるオペランドから型を決める。比較のi32結果はオペランドの幅を決めないので、裸の数値だけなら`(< (i64.const 1) 3)`と一方を明示する。利用者定義class/implはなく、単相化後にWasm命令へ直接展開する。[詳しい規約](docs/typeclasses.md)。
`&&`・`||`の入力は条件式と同じi32。右辺も静的検査するが、必要な場合だけ実行する。シフト量は左辺と同じ型とし、Wasm同様に下位5bit（i32）/6bit（i64）を使う。`(Logical $T)`・`(Bitwise $T)`・`(Shift $T)`も`@where`に指定できる。
`Sub`・`Mul`・`Div`も同様に制約に指定する。整数の`/`は0方向へ丸め、ゼロ除算と最小値/-1でtrapする。floatの`/`はWasmの浮動小数点除算に従う。

期待型が `$string` の引数・結果・フィールド・list要素・型付きletでは、裸の文字列も使える。期待型のないlet初期値には `(type $string)` か `@string.lit` を書く。別の型へは変換しない。[Option/Resultの関数と文字列の規約](docs/option-result.md)。

## GC データと enum

```text
(type $Cell (struct (field $value (mut i64))))
(struct.new $Cell value)
(struct.get $Cell $value object)
(struct.set $Cell $value object value)

(type $Ints (array (mut i64)))
(array.new $Ints initial-element length)
(array.new_fixed $Ints 2 first second)
(array.get $Ints array index)
(array.set $Ints array index value)
(array.len array)
(ref.null $Cell)
(ref.as_non_null nullable)           ;; null なら trap。暗黙の型絞り込みはない
(ref.is_null reference)
(ref.eq reference reference)         ;; 構造比較でなく参照同一性
```

名前付き構築は全フィールド必須。重複・欠落・未知の名前を拒否し、初期値は記述順に一度ずつ評価する。
`{}`と裸リテラルは独立した糖衣で、型・effect・所有権の検査を保つ。[詳細](docs/struct-construction.md)。

```text
(@struct.new $User ($name (@string.lit "Alice")) ($age (i32.const 30)))
(@struct.new $User ($name "Alice") ($age 30))
(@struct.new $User {:name (@string.lit "Alice") :age (i32.const 30)})
(@$User::{:name "Alice" :age 30})
(@struct.new (@inst $Box i64) {:value 42})
```

```wat
(module
  (type $Item (@variant (none) (count i64)))
  (func $get
    (param $item (ref $Item))
    (result i64)
    (@match $Item (result i64)
      (local.get $item)
      (case none () (i64.const 0))
      (case count ($n) (local.get $n))))
  (@test "variant"
    (i64.eq (call $get (@variant.new $Item count (i64.const 42))) (i64.const 42))))
```

全ケースを1回ずつ書く。payload のない case も `()` が必要。型・個数・網羅性を検査する。
通常の enum は `(ref $Item)`、Option/Result/pair は参照を直接操作しない値型。

## Option / Result / pair

```text
(@option.none T)
(@option.some T value)
(@result.ok T E value)
(@result.err T E error)
(@tuple.new T U first second)

(@match (@inst $option T) (result R) subject
  (case none () no-value) (case some ($value) with-value))
(@match (@inst $result T E) (result R) subject
  (case ok ($value) success) (case err ($error) failure))
(@match (@inst $tuple T U) (result R) subject
  (case pair ($first $second) body))
```

unit の成分は値引数と payload 束縛を省略する。例：`(@result.ok $unit i32)`、`(case ok () body)`。
Result を返す関数は `(result (@inst $result T E))`。外側は宣言、内側は値型。

pair の状態を Result で返す例。match の `(result (@inst $result ...))` も外側が宣言、内側が値型である。
以下は明示形で、構築・分解する位置ごとに型を書く。型の alias と省略形も使える。

```wat
(module
  (@use "walz:core" (type $tuple) (type $result))
  (func $increment
    (param $state (@inst $tuple i64 i32))
    (result (@inst $result (@inst $tuple i64 i32) i32))
    (@effect pure)
    (@match (@inst $tuple i64 i32) (result (@inst $result (@inst $tuple i64 i32) i32))
      (local.get $state)
      (case pair ($id $count)
        (@result.ok (@inst $tuple i64 i32) i32
          (@tuple.new i64 i32 (local.get $id) (i32.add (local.get $count) (i32.const 1))))
      )))
  (@test "pair state"
    (@match (@inst $result (@inst $tuple i64 i32) i32) (result i32)
      (call $increment (@tuple.new i64 i32 (i64.const 7) (i32.const 41)))
      (case ok ($state)
        (@match (@inst $tuple i64 i32) (result i32)
          (local.get $state)
          (case pair ($id $count) (i32.eq (local.get $count) (i32.const 42))))
      )
      (case err ($error) (i32.const 0))))
)
```

### result.let：成功時だけ続ける

```text
(@result.let ($value) (@inst $result T E) U subject body)
(@result.let ($value) subject body)    ;; 入力型と出力成功型を省略する形
```

subject は result<T,E>、body は result<U,E>、式全体も result<U,E>。
どちらの形も最後の2式が subject と成功時の body。型を省略しても評価順・スコープ・err の伝播は同じ。
引数の位置は次のとおり。**裸の U は出力の成功型だけ**で、入力型やエラー型ではない。

| 位置 | 指定するもの |
|---|---|
| `($value)` | subject の成功 payload を body 内だけで使う名前 |
| `(@inst $result T E)` | 入力 subject の値型 |
| `U` | body と式全体の成功型。エラー型 E は入力から引き継ぐ |
| `subject` / `body` | 入力の式／成功時だけ評価する式 |

T=unit なら `($value)` の代わりに `()`。初版の E は共有値。
err なら body を評価せず、同じ E の err をこの式の結果にする。**関数からの暗黙 return ではない。**

unit 入力の `()` は「成功payloadに名前を付けない」。成功時の継続bodyは省略しない。

```wat
(module
  (@use "walz:core" (type $result) (type $unit))
  (func $after-unit
    (param $input (@inst $result $unit i32))
    (result (@inst $result i64 i32))
    (@effect pure)
    (@result.let ()
      (local.get $input)
      (@result.ok (i64.const 42))))
  (@test "unit success continues"
    (@match (call $after-unit (@result.ok $unit i32))
      (case ok ($n) (i64.eq (local.get $n) (i64.const 42)))
      (case err ($e) (i32.const 0))))
  (@test "unit error propagates"
    (@match (call $after-unit (@result.err $unit i32 (i32.const 7)))
      (case ok ($n) (i32.const 0))
      (case err ($e) (i32.eq (local.get $e) (i32.const 7))))
  ))
```

```wat
(module
  (@use "walz:core" (type $result))
  (func $convert
    (param $input (@inst $result i64 i32))
    (result (@inst $result i32 i32))
    (@effect pure)
    (@result.let ($n) (@inst $result i64 i32) i32
      (local.get $input)
      (@result.ok i32 i32 (i32.wrap_i64 (local.get $n))))
  )
  (@test "result mapping"
    (@match (@inst $result i32 i32) (result i32)
      (call $convert (@result.ok i64 i32 (i64.const 42)))
      (case ok ($n) (i32.eq (local.get $n) (i32.const 42)))
      (case err ($e) (i32.const 0))))
)
```

## Future / Stream と終了処理

```text
(@future.ready T value)
(@future.await T (@move $future))       ;; future を消費し T を返す
(@future.close T (@move $future))
(@stream.empty T)
(@stream.cons T value rest-stream)    ;; rest-stream も消費する
(@stream.next T (@move $stream))       ;; pair<stream<T>, option<T>>
(@stream.close T (@move $stream))
```

await/next は常に suspends。EOF の next も端点を返すので、最後は close が必要。
payload は共有できる値に限定する。端点自身は共有 GC フィールドに保存しない。
通常の core 実行は配送済みデータ、Core＋JSPI / Component は Promise と外部 Stream にも接続する。

### stream.fold-result：失敗時も閉じる反復

```text
(@stream.fold-result T S E source initial ($state $item) body)
(@stream.fold-result source initial ($state $item) body)
```

source は stream<result<T,E>>、initial は S、body は result<S,E>。全体は result<S,E>。
短い形は先頭の型引数 T / S / E を省略するだけで、source・initial・束縛・body の順は同じ。
source、initial の順に一度だけ評価する。S は共有できる非 unit の状態。
EOF は close して ok(state)、入力の err／body の err は close して err(E)。
T=unit の束縛は `($state)` だけ。body は同じ関数の loop/match へ展開し、closure は生成しない。
任意の throw に対する finally ではない。取消しの終了確認は既存の task scope が担当する。

| 終了の理由 | fold と呼出し元の動作 |
| --- | --- |
| EOF | 入力を close し、最後の状態の ok を返す |
| 入力または行処理の err | 入力を close し、err を返す。外側の成功bodyは評価しない |
| JS からの取消し | ホストの `start` が作った task scope が待機中の操作を中止し、解放完了後に終了を確認する。Result の err へは変換しない |
| Wasm の throw / trap | Result と別の経路。fold が任意の例外で close する保証はない。trap は catch できない |

`@task.scope (result T) body` は言語内の子タスクを束ね、退出時に完了を待つ。`@task.cancel T handle` は子と孫の取消し・解放を待ち、親と兄弟を継続する。`@task.join-timeout T ms handle` は完了なら `some`、期限切れの解放後は `none` を返す。両操作は handle を消費する。[子タスクの契約](docs/scoped-tasks.md) を参照。

JS ホスト側にも公開呼出し単位の task scope がある。`task.cancel()` は要求後に解放完了を待つ Promise、`task.result` は取消し時に `TaskCancelledError` で reject する。取消しは待機点での協調的な処理で、CPUだけの処理の強制停止や既存の変更の巻き戻しは行わない。実行例は [タスクの取消し](docs/runtime.md#タスクの取消し) を参照。

## 型の alias と省略形

`(@type.alias $State (@inst $tuple i64 i32))` は透明な型の略記。型の位置に `$State` と書く。
GC オブジェクトや nominal identity は増えない。`(ref $State)` のような heap 型名には使わない。
forward 参照可、循環は禁止。ファイルの公開は `(@export (type $State))`、use も type と同じ。

関数の結果、引数、明示した let 型などから期待型が決まれば、値の構築に型引数を繰り返さない。
決まらない場合は型注釈を追加する。関数・if・block・明示形 match の結果宣言省略は従来どおり unit。

```wat
(module
  (@use "walz:core" (type $tuple) (type $result))
  (@type.alias $State (@inst $tuple i64 i32))
  (func $increment
    (param $state $State)
    (result (@inst $result $State i32))
    (@let ($id $count) (local.get $state)
      (@result.ok
        (@tuple.new (local.get $id) (i32.add (local.get $count) (i32.const 1))))
    ))
  (@test "short state"
    (@match (call $increment (@tuple.new (i64.const 7) (i32.const 41)))
      (case ok ($state)
        (@let ($id $count) (local.get $state)
          (i32.eq (local.get $count) (i32.const 42))))
      (case err ($error) (i32.const 0))))
)
```

| 省略形 | 型の決め方 |
|---|---|
| `(@result.ok value)` / `(@result.err error)` | 期待する Result 型。unit payload の値引数は省略 |
| `(@tuple.new first second)` | 期待する pair 型。unit 成分の値引数は省略 |
| `(@option.some value)` / `(@option.none)` / `(@box.new value)` | 期待する Option / Own 型 |
| `(@let ($first $second) subject body)` | subject の pair を一度だけ評価して分解。unit 成分の名前は省略 |
| `(@match subject (case ...)... )` | subject の型と期待する出力型。型付きの長い形も使用可 |
| `(@result.let ($value) subject body)` | subject の T/E と、期待型または body で局所的に決まる U |
| `(@stream.fold-result source initial ($state $item) body)` | source の T/E、initial または期待型の S。body は result<S,E> |

省略しても評価順、消費義務、effect、opaque の検査は同じ。result.let の E は共有かつ入力と出力で同じ型。fold の E も共有値、S は共有できる非 unit に限る。
pattern let で own 成分を含む pair を分解するときは `(@move $tuple)` を使い、取り出した所有値も消費する。
任意の型の穴や関数をまたぐ全域型推論、暗黙の move / capture は導入しない。

## 所有値・クロージャ・effect

汎用関数・型・alias・クロージャは名前の直後に `(@type.params $T ...)` を宣言し、
`(@inst $name T...)` で具体化する。通常の関数の `call` では一意に決まる型引数を省略でき、具体化後も型・effect・所有権を検査する。
具体的なクロージャ型も渡せる。[詳細と map/fold](docs/generics.md)。

```wat
(module
  (func $identity
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (param $x $T)
    (result $T)
    (@effect pure)
    (local.get $x))

  (func $main
    (export "main")
    (result i64)
    (@effect pure)
    (call (@inst $identity i64) (i64.const 42))))
```

type.params の直後に `(@where (shared $T) (nonunit $T))` を置くと、具体化する型引数を制限できる。
共有は不変性を意味しない。コールバックには
`(callable $F (param $T) (result $U) (@effect pure))` と書ける。
引数・戻り値が一致する共有・再呼出し可能なクロージャに限定し、effect は本体から推論する。
effects を省略すれば effect の制限は課さない。未使用の適用も DCE 前に検査する。
明示的な適用は `(@inst $name T...)`。関数では `(call (@inst $identity i64) value)`、汎用型では `(ref (@inst $Box i64))`、汎用 alias では型の位置に `(@inst $Values i64)` と書く。
関数呼び出しでは引数・期待型・callable のシグネチャから型引数を決める。曖昧なら `E_GENERIC_INFERENCE`。数値幅は推測せず、実装探索は行わない。型・effect・所有権は具体化した本体で検査し、未適用の汎用本体が任意の型引数で正しいという保証は与えない。

```text
(@box.new T value)
(@move $owner)                       ;; 所有値を消費する位置に渡す
(@box.close (@move $owner))            ;; 包みを消費。OS resource の destructor ではない

(@closure $Adder (@capture $base i64) (param $x i64) (result i64)
  (i64.add (local.get $base) (local.get $x)))
(@closure.new $Adder (i64.const 40))
(@closure.call $Adder closure (i64.const 2))
```

所有値に local.get、drop、move 後の再利用は不可。Option/Result/pair に所有成分があれば全体も所有値。
own は payload の GC 参照の別名を禁止しない。GC の回収と外部端点の終了処理は別。
closure は capture を全て宣言し、構築時に捕捉値を渡す。所有 capture の closure は1回だけ呼べる。

```wat
(module
  (@closure $Adder
    (@capture $base i64)
    (param $x i64)
    (result i64)
    (i64.add (local.get $base) (local.get $x)))

  (func $main
    (export "main")
    (result i64)
    (@let $add40 (@closure.new $Adder (i64.const 40))
      (@closure.call $Adder (local.get $add40) (i64.const 2))))
  (@test "explicit capture" (i64.eq (call $main) (i64.const 42))))
```

| effect | 意味 |
|---|---|
| `pure` | 以下の semantic effect が空。停止・無 trap・物理的割当てなしの証明ではない |
| `reads writes` | 可変 GC データの読み書き。ローカルな作業配列も含む保守的な上限 |
| `allocates` | identity を観測できる GC 値の生成。値型の格納用割当てとは区別 |
| `host suspends` | ホスト作用／中断境界。別々に検査する |
| `(throws $Tag)` | 関数外へ出る可能性のある Wasm 例外 |

effect 注釈は上限で、実際の本体から検査する。省略時は推論。公開型が opaque でも変更メソッドの effect は隠れない。

## ループ・Wasm 例外

```text
(@loop $go (bind $i i32 initial)... (result R) body)
(@recur $go next-value...)            ;; 最内側 loop の末尾だけ。同時更新
(@for $go (bind $i i32 initial)...
  (while condition) (step next-value...) unit-body)

(tag $Error (param i32))
(@try (result R)
  (do expression)
  (catch $Error ($code) handler)
  (catch_all handler))
(throw $Error payload...)
```

try/catch は Wasm 例外の糖衣。ゼロ除算・境界違反等の trap は catch_all で捕捉しない。
所有値を持つ関数内の try/throw は初版で制限されるため、その失敗には Result を使う。

## モジュール・不透明な公開型

WIT付きComponentは`(@use "./math.wasm" (func $math.add $add))`で選択できる。型はバイナリから取得する。初版はホストimportなし・同期・共有値の関数に限定し、呼出しには`host` effectが必要。出力は`--target component`。[制限と例](docs/wasm-use.md)。

```wat
;; stock.walz: 1ファイルが1モジュール、宣言は既定で非公開
(module
  (@export (opaque $State) (func $new) (func $get))
  (type $State (struct (field $n i32)))
  (func $new (result (ref $State)) (struct.new $State (i32.const 42)))
  (func $get (param $s (ref $State)) (result i32)
    (struct.get $State $n (local.get $s))))
```

```wat
;; main.walz: 対象と別名を明示して選択
(module
  (@use "./stock.walz" (type $State $Alias) (func $new) (func $get $read))
  (func $main (export "main") (result i32) (call $read (call $new))))
```

`@namespace` はない。ファイルの `@use` は `./`・`../` で始まる相対パス、組み込みは `walz:core` などのパッケージ名を使う。
ドットは識別子の一部で、名前空間を探索しない。選んだ名前だけが束縛され、同じファイルの型・定義は共有する。別名も `@export` で再公開できる。
`@export (type $T)` は表現も公開し、`@export (opaque $T)` は型だけを公開する。opaque の構築・フィールド／配列操作・match は宣言ファイル内だけ。別名経由も同じ。
`@public` は `@export` の同義語。ネイティブWATの `(import "host" "member" ...)` と `(export "main" ...)` はWasm ABI用で、静的な `@use` / `@export` と区別する。
`@include "relative.walz"` は宣言のテキスト挿入で、スコープを作らない。使わない定義は検査後に生成時に落ちる。
[境界・DCE・未対応範囲](docs/static-modules.md) を参照。

## bytes / text とライブラリ

| 式・関数 | 結果 |
|---|---|
| `(@bytes.new i32-expr...)` | 不変 bytes |
| `(@bytes.from-array $Array array)` | 非 null の i32 配列をコピー。各要素0..255以外は trap |
| `(@bytes.len b)` / `(@bytes.get b index)` | 長さ／0..255の要素。添字は符号なし、範囲外は trap |
| `(@bytes.slice b start length)` | コピーした bytes。範囲外は trap |
| `(@string.lit "日本語")` / `(@string.byte-len t)` | text／UTF-8 byte 長 |
| `(@string.bytes t)` / `(@string.from-bytes b)` | bytes／option<text>。不正 UTF-8 は none |
| `$string.concat(text,text)` | text の連結 |
| `$string.u64(i64)` / `$string.s64(i64)` | ビットパターンの符号なし／符号付き十進数化 |
| `$json.from-string(text)` | JSONのstring値を構築（文書の解析は `$json.parse`） |
| `$json.stringify(value)` | JSON値をJSONテキストとして出力 |

標準ライブラリの利用入口は `walz:core` に統一する。例えば `(@use "walz:core" (type $string) (func $json.from-string) (func $json.stringify))` で取り込み、`(call $json.stringify (call $json.from-string text))` でJSON文字列リテラルを出力する。内部の作業配列を扱う関数は reads/writes/allocates を持つ。
`walz/core/*.walz` は同梱する実装のファイル。利用者の宣言群を挿入する `@include "program.walz"` とは役割を分ける。`@include` は同じスコープへのテキスト挿入であり、暗黙に標準名を提供する仕組みではない。

検索・分割・整数解析には `$bytes.find-byte`、`$string.find`、`$string.split-once`、`$string.parse-u64` / `$string.parse-s64` を使う。
`parse-u64` は入力全体の ASCII 数字を検査し、符号なし64bitの範囲で some(i64) を返す。空・負数・`+`・空白・桁あふれは none。先頭ゼロは許す。成功値は i64 のビットパターンで、`string.u64` / `builder.u64` で符号なし十進数へ戻す。
位置は UTF-8 byte 単位。`$bytes.parse-u64(bytes,start,length,limit)` でアプリ固有の上限も指定でき、不正な入力は none になる。
[全 API と境界の規約](walz/core/README.md) を参照。JSON object はキー・値・区切りを自動でエスケープ／出力できる。

```wat
(module
  (@use "walz:core" (type $string))
  (@use "walz:core"
    (func $json.object-builder.new)
    (func $json.object-builder.string)
    (func $json.object-builder.u64)
    (func $json.object-builder.finish))
  (func $record
    (export "record")
    (param $name $string)
    (param $count i64)
    (result $string)
    (@effect reads writes allocates)
    (@let $o (call $json.object-builder.new)
      (block
        (result $string)
        (call $json.object-builder.string (local.get $o) (@string.lit "name") (local.get $name))
        (call $json.object-builder.u64 (local.get $o) (@string.lit "count") (local.get $count))
        (call $json.object-builder.finish (local.get $o))))
  ))
```

`$json.object-builder.finish` は内容を返して空 object に戻す。text 断片を組む場合は `walz/core/builder.walz` の `$string-builder.new` / `$string-builder.text` / `$string-builder.finish` を使う。

## 確認コマンド

インストール済みのCLIからプロジェクトを作ります。コンパイラのcheckoutは不要です。
実行にはWasm GC対応のWasmtime、ベンチとCore＋JSPI実行にはNode.js 24以降が必要です。

```sh
walz new app
cd app
walz check src/main.walz
walz run                           # 42
walz build                         # target/main.wasm
walz test
walz bench --list
walz bench --samples 10
walz types src/main.walz
walz expand src/main.walz
walz fmt-check src/main.walz
```

JSへの配布は`walz build --target core-js -o dist/app`、ポータブルな戻り値の確認は`walz run --target core-js`を使います。
Core WasmのGC参照はstringへ自動変換されません。I/Oプログラムでは`--stdio`で標準入出力を明示的に許可し、
`--input input.txt`で入力、`--output output.txt`で出力先を指定します。これらのフラグは戻り値をraw textに変えるものではありません。
`walz run`でmainのResultがerrなら、失敗終了になります。

`(@test "name" i32-expr)`は0なら失敗、非0なら成功。`@law`は別の契約で、対応する純粋な整数式の全入力をLeanで検査します。
`*_bench.walz`の`@bench`は`walz bench`が見つけます。反復引数と結果型は省略可能です。
計測する値を返し、入力を変えて測定します。時間の範囲・ウォームアップ・JSON出力は[ベンチマーク](docs/benchmarking.md)を参照してください。
既存のtext I/O capabilityでは、read-linesはstream<result<text,i32>>、write-lineはfuture<result<unit,i32>>を返します。

コンパイラcheckoutを使うタスクは[開発](docs/development.md)、低水準のホストAPIは[ランタイム](docs/runtime.md)へ分けています。
全域型推論・利用者定義trait・手続き的マクロ・一般的なborrow lifetime・無制限のWASI bindingは提供しません。

## 数値・平坦な Result 連鎖・newtype・早期脱出

スカラーの変換・飽和変換・丸め・sqrt・min/max・reinterpret は標準 WAT の命令名をそのまま使う。
`i32.trunc_f64_s` は範囲外と NaN で trap、`i32.trunc_sat_f64_s` は飽和変換。

```text
(type $UserId (@newtype i64))
(@newtype.wrap $UserId scalar)        ;; 型検査後に消去。基底はi32/i64/f32/f64
(@newtype.unwrap $UserId id-value)    ;; 異なるnewtypeや数値への暗黙変換はない

(@result.do
  ($a first-result)
  ($b second-result)
  (() unit-success-result)
  final-result)

(return [value])
(block $done [(result R)] body...)
(br $done [value])
(br_if $done [value] condition)       ;; falseならvalueが残る
```

`@result.do` は1個以上の束縛と最後の Result 式。左から右に一度ずつ評価し、最初の err で残りを省略してその式の err を返す。エラー型はすべて同じ。`$a` は後の束縛から参照でき、unit 成功は `()` に束縛する。暗黙に関数から return しない。

```wat
(module
  (@use "walz:core" (type $result))
  (type $Id (@newtype i32))
  (func $compute (result (@inst $result $Id i32)) (@effect pure)
    (@result.do
      ($a (@result.ok i32 i32 (i32.const 40)))
      ($b (@result.ok i32 i32 (i32.const 2)))
      (@result.ok $Id i32
        (@newtype.wrap $Id (i32.add (local.get $a) (local.get $b))))))
  (func $main (export "main") (result i32) (@effect pure)
    (@match (call $compute)
      (case ok ($id) (return (@newtype.unwrap $Id (local.get $id))))
      (case err ($code) (i32.const 0))))
  (@test "newtype and flat results" (i32.eq (call $main) (i32.const 42))))
```

名前付き block の分岐値と return 値は宣言した結果型に合わせる。`br_if` は値を条件より先に評価する。
`@using` を抜ける return・分岐・反復は、値の評価後に抜ける scope だけを逆順に close する。
所有値と try/catch を併用できるが、全経路で消費する。後続の引数が例外を投げて先の所有引数が取り残される場合も拒否する。
newtype は Walz 内で型を区別し、JavaScript/WIT 境界では基底の数値になる。値域検査や JavaScript の brand を自動では追加しない。
[詳しい制約と例](docs/core-expressions.md)。

## 共有メモリと atomics

```text
(import "env" "memory" (memory $shared 1 2 shared))
(i32.atomic.load $shared offset=0 align=4 address)
(i64.atomic.store $shared address value)
(i32.atomic.rmw.add $shared address value)
(i64.atomic.rmw.cmpxchg $shared address expected replacement)
(memory.atomic.wait32 $shared address expected timeout-i64)
(memory.atomic.notify $shared address count-i32)
(atomic.fence)
(memory.size $shared)
(memory.grow $shared pages-i32)
```

atomic load は `reads synchronizes`、store は `writes synchronizes`、RMW は `reads writes synchronizes`。wait は `reads synchronizes blocks`、notify/fence は `synchronizes`。wait の timeout はナノ秒、結果は notified=0 / not-equal=1 / timed-out=2。`blocks` は JSPI の `suspends` と別。
アドレスは i32、値は命令に応じて i32/i64。GC ref や所有値は格納できない。既存の move 検査は維持するが、領域の排他所有や race-free は証明しない。
共有メモリをホストと渡すには Core / Core＋JSPI を使う。[全命令・型・制約・Worker 例](docs/threads.md)。

## 固定 atomic セルの検証モード

```text
(@race-free)
(import "env" "memory" (memory $m 1 1 shared))
(@atomic.cell $counter $m i32 0)
(i32.atomic.rmw.add (@atomic.at $counter) value-i32)
```

メモリは1つ・固定サイズ・shared、セルは重複しないi32/i64の固定幅。生のアドレス、狭幅アクセス、余分なoffset、size/grow、ホスト関数importを拒否する。`@atomic.at`は整数として取り出せない。セルは`@export (cell $name)` / `@use "./file.walz" (cell $name $alias)`で別名を付けられる。
ネイティブのビルドでLeanカーネルを実行する。ホストの契約順守を前提にデータ競合の排除・同一セル幅・範囲を証明し、更新喪失やdeadlockは証明しない。Core＋JSPIのホスト共有では `atomicHostContract: 'walz-atomic-cells-v1'` が必要。[仕様と証明範囲](docs/atomic-cells.md)。

## 読み取り専用の共有領域

```text
(@readonly.region $index $m byte-offset positive-byte-length)
(@readonly.load $index u8 relative-byte-offset-i32)
(@readonly.load $index i32 relative-byte-offset-i32)
(@readonly.load $index i64 relative-byte-offset-i32)
(@readonly.load $index v128 relative-byte-offset-i32)
```

offsetを一度評価して領域内を検査し、非atomicに読む。`reads`が必要。`@race-free`と併用すると、公開後に書かないホスト契約の下でatomicセルと分離した読取りを検証する。追加の定義済み私有memoryを使える。Core＋JSPIでは`readonlyHostContract: 'walz-readonly-regions-v1'`を指定する。[詳細](docs/readonly-regions.md)。[汎用Workerホスト](docs/worker-host.md)はJSから明示的に使い、`@task.spawn`を別スレッド化するものではない。

## Option / Result の標準関数

`(@use "walz:core" (func $option.ok-or) (func $result.map-err))` で明示的に取り込む。
`(call (@inst $option.ok-or T E) optional error)` はsome→ok、none→err。errorは常に先に評価する。
`(call (@inst $result.map-err T E U $Closure) result-value mapper)` はerrにだけclosureを一度適用し、okを保つ。
T/E/Uはshared・nonunit、closureはE→U。unitや所有payloadには `@match` を使う。[契約と実行例](docs/option-result.md)。
