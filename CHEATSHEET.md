# wlt 文法チートシート

標準 WAT を基底に、型付きの式・GC データ・所有値・固定マクロを追加した言語。
ここでは拡張を使う型付きコアを扱う。WLT の拡張入口には `%` を付ける。標準 WAT 単体はそのまま受理し、コア外の WAT と構文マクロの混在は opaque として全体を検証する。[構文の境界](docs/syntax.md) を参照。
詳しい意味論は [SPEC.md](SPEC.md)、実用例は [在庫台帳](examples/apps/stock-ledger/README.md)。

## 最初に覚える規則

- 式は `(操作 引数...)`。式位置の `$foo` は `(local.get $foo)`。数値は期待型が分かれば `42`、明示するなら `(i64.const 42)`。文字列は `(%text.lit "hello")` と書く。
- プログラムは `(module 宣言...)`。関数の本体は1つの式。複数の処理には block を使う。
- `$name` は識別子、`"text"` は文字列、`;;` は行コメント、`(; ... ;)` は入れ子可能なコメント。
- 引数と演算は左から右へ評価する。暗黙の数値変換・捕捉・ローカル再代入はない。
- **`(result R)` は結果型の宣言。`(%inst $result T E)` は成功 T／エラー E の値型。**
- 結果宣言を省略すると unit。本体から結果型を推論する指定ではない。
- 共有 GC 参照と、消費が必要な own / Future / Stream を区別する。

以下の `T U E S`、`expr`、`body`、`...`、`[省略可能]` は説明用の記号。字面をソースに書くものではない。
`wat` のコードブロックはそれぞれコンパイルできる完全なモジュール、`text` は構文のひな形。

## 標準 WAT と `%` 拡張の境界

| 入口 | 意味 |
|---|---|
| `func`・`param`・`result`・`if`・`block`・`call` | 標準 WAT の関数・型宣言・式。`if` に `%` は付けない |
| `%let`・`%for`・`%loop`・`%recur`・`%match`・`%try` | WLT の束縛・制御構文。標準の `loop` / `try` と区別する |
| `(result i64)` / `(result i32 i64)` | 標準 WAT の結果型宣言。後者は標準 WAT の複数戻り値 |
| `(%inst $result T E)` | 成功 T／エラー E を持つ WLT の値型。返す関数は `(result (%inst $result T E))` |
| `(import "host" "member" ...)` / `(export "name" ...)` | Wasm のインスタンス化時の import／生成モジュールの export |
| `(%import "file.wlt" ...)` / `(%export ...)` | WLT ソースの選択的な静的リンク／ソース API の公開 |
| `(%import "wlt:std" ...)` | コンパイラに同梱した標準型・標準関数を名前ごとに取り込む |
| `%closure`・`%capture`・`%effect`・`%type.params`・`%where` | WLT が追加した宣言・注釈 |

`%` は拡張の入口に付ける。内部の `case`、`bind`、`while`、`step`、`do`、`catch`、制約の `shared` / `nonunit` / `callable` は、それぞれの入口に属する構文なので接頭辞を付けない。識別子は従来どおり `$name`。

標準 WAT 単体はそのまま検証・コンパイルする。以下の型付きコアでは型・effect・所有権を検査する。
コア外の WAT 命令と混在できる拡張は、構文だけで展開できる `%let` / `%loop` / `%for` などに限る。その経路は Wasm 全体を検証し、`analysis.opaque=true` と保守的な effect を返す。`%effect pure` を付けて詳細な解析や所有権の保証を得ることはできず、検査を必要とする拡張との併用は受理しない。
型付きコアの戻り値は0個（unit）または1値。pair も1値として返す。標準 WAT の `(result i32 i64)` は標準 WAT 経路で扱い、型付きコアの pair 分解へは変換しない。

## `wlt:std` と基本データ型

WLT の基本型は `wlt:std` から明示的に import する。`i32` などの Wasm の型はそのまま使う。

```text
(%import "wlt:std"
  (type $result) (type $option) (type $future) (type $stream)
  (type $pair) (type $list) (type $own)
  (type $text) (type $bytes) (type $unit)
  (func $string.u64 $format))

(result (%inst $result $text i32))
(param $pending (%inst $future (%inst $result $text i32)))
(call $format (i64.const 42))
```

型も汎用関数も `(%inst $name T...)` で適用する。`$text` / `$bytes` / `$unit` は型引数なしの alias なので、その名前で参照する。`(type $result $Outcome)` のような別名も使える。
名前は import したスコープにだけ束縛される。暗黙の prelude はなく、別ファイルや namespace で使う名前もそこで import する。標準関数は `$string.*`、`$buffer.*`、`$list.map` / `$list.fold`、`$builder.*`、`$json.*`、`$json-value.*` から選ぶ。`wlt:std` はファイルパスやホスト import ではなく、通常のライブラリの静的リンクと DCE を使う。
`%text.lit`・`%result.ok`・`%future.await` などは値を操作するコンパイラの固定構文。型の参照には import した名前と `%inst` を使う。
関数の完全な型・型制約・effect は、公開宣言から生成する [標準 API 一覧](docs/stdlib-api.md) を参照する。
`$dict.*` はtextキーの汎用辞書、`$list-builder.*` は不変listの構築。どちらも共有可能・非unitの要素を明示的に型適用し、内部表現はopaqueにする。[コレクション](docs/collections.md) を参照。

```wat
(module
  (%import "wlt:std" (type $future))
  (func $main
    (export "main")
    (result i64)
    (%effect suspends)
    (%let
      (($pending (type (%inst $future i64)) (%future.ready i64 40))
       ($base (%future.await i64 (%move $pending)))
       ($answer (type i64) (i64.add $base 2)))
      $answer))
  (%test "standard Future" (i64.eq (call $main) (i64.const 42))))
```

## 関数・型・束縛

```wat
(module
  (func $add
    (param $a i64)
    (param $b i64)
    (result i64)
    (%effect pure)
    (i64.add (local.get $a) (local.get $b)))

  (func $main
    (export "main")
    (result i64)
    (%effect pure)
    (%let $answer (call $add (i64.const 40) (i64.const 2))
      (local.get $answer)))
  (%test "answer" (i64.eq (call $main) (i64.const 42))))
```

| 型 | 意味 |
|---|---|
| `i32 i64 f32 f64` | Wasm の数値型。整数の加減乗算は折り返し、整数のゼロ除算等は trap。符号解釈は命令の `_s` / `_u` で指定 |
| `v128` | Wasm の128ビット値。`v128.const`、splat、lane操作、算術・比較・shuffle等は標準命令名を使う。[SIMD](docs/simd.md) |
| `(ref $T)` / `(ref null $T)` | GC の非 null／nullable 参照。型は nominal |
| `$bytes` / `$text` | 不変なバイト列／妥当な UTF-8。内部参照の操作や比較はできない |
| `(%inst $list T)` | 共有値を要素とする不変リスト |
| `(%inst $option T)` / `(%inst $result T E)` | none/some、ok/err の値型 |
| `(%inst $pair T U)` | 2成分の値。成分が共有値なら pair も共有値 |
| `(%inst $own T)` / `(%inst $future T)` / `(%inst $stream T)` | 消費する所有値。GC 参照への排他的所有を意味しない |
| `$unit` | 値のない結果。関数の引数・通常のフィールドには格納しない |

```text
(func $f (param $x T)... [(result R)] [(%effect ...)] body)
(%let $x [(type T)] initializer body) ;; 注釈省略時は初期値の型。不変の字句束縛
(%let (($a [(type T)] initial-a) ($b [(type U)] initial-b) ...) body)
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

複数束縛の `%let` は、初期値を左から右に一度ずつ評価する。後の初期値と body からは前の束縛を参照でき、後の束縛の先行参照はできない。同名の再束縛は、その初期値の評価後から以前の名前を隠す。個々の型注釈は省略できる。これはネストした `%let` と同じ意味であり、Wasm の複数戻り値を分解する指定ではない。
pair の分解には既存の `(%let ($a $b) subject body)` を使う。どちらの形式も型・effect・所有権を検査し、所有値には明示的な move と消費が必要。

```wat
(module
  (func $main (export "main") (result i32)
    (%let $foo (type i32) 41
      (i32.add $foo 1))))
```

`1` の型は演算命令・引数・戻り値・フィールド・`(type T)`注釈などから決める。既定の整数幅は設けず、`(%let $x 1 ...)` と `(drop 1)` は曖昧なのでエラー。`$foo`は既存のローカルを読むだけで、暗黙のmoveや変換はしない。[省略式の規約](docs/core-expressions.md#ローカル参照と数値の省略形)。

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
  (func $add (%type.params $T) (%where (Add $T))
    (param $a $T) (param $b $T) (result $T) (%effect pure)
    (+ $a $b))
  (func $main (export "main") (result i64)
    (call (%inst $add i64) 40 2))
  (%test "literal plus" (= (call $four) 4))
  (%test "short circuit" (|| (= (call $four) 4) (unreachable)))
  (%test "masked shift" (= (i64.const 1) (<< (i64.const 1) 64))))
```

固定の記号演算子には`%`を付けない。`+`は期待型または型の分かるオペランドから型を決める。比較のi32結果はオペランドの幅を決めないので、裸の数値だけなら`(< (i64.const 1) 3)`と一方を明示する。利用者定義class/implはなく、単相化後にWasm命令へ直接展開する。[詳しい規約](docs/typeclasses.md)。
`&&`・`||`の入力は条件式と同じi32。右辺も静的検査するが、必要な場合だけ実行する。シフト量は左辺と同じ型とし、Wasm同様に下位5bit（i32）/6bit（i64）を使う。`(Logical $T)`・`(Bitwise $T)`・`(Shift $T)`も`%where`に指定できる。
`Sub`・`Mul`・`Div`も同様に制約に指定する。整数の`/`は0方向へ丸め、ゼロ除算と最小値/-1でtrapする。floatの`/`はWasmの浮動小数点除算に従う。

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
(%struct.new $User ($name (%text.lit "Alice")) ($age (i32.const 30)))
(%struct.new $User ($name "Alice") ($age 30))
(%struct.new $User {:name (%text.lit "Alice") :age (i32.const 30)})
(%$User::{:name "Alice" :age 30})
(%struct.new (%inst $Box i64) {:value 42})
```

```wat
(module
  (type $Item (%variant (none) (count i64)))
  (func $get
    (param $item (ref $Item))
    (result i64)
    (%match $Item (result i64)
      (local.get $item)
      (case none () (i64.const 0))
      (case count ($n) (local.get $n))))
  (%test "variant"
    (i64.eq (call $get (%variant.new $Item count (i64.const 42))) (i64.const 42))))
```

全ケースを1回ずつ書く。payload のない case も `()` が必要。型・個数・網羅性を検査する。
通常の enum は `(ref $Item)`、Option/Result/pair は参照を直接操作しない値型。

## Option / Result / pair

```text
(%option.none T)
(%option.some T value)
(%result.ok T E value)
(%result.err T E error)
(%pair.new T U first second)

(%match (%inst $option T) (result R) subject
  (case none () no-value) (case some ($value) with-value))
(%match (%inst $result T E) (result R) subject
  (case ok ($value) success) (case err ($error) failure))
(%match (%inst $pair T U) (result R) subject
  (case pair ($first $second) body))
```

unit の成分は値引数と payload 束縛を省略する。例：`(%result.ok $unit i32)`、`(case ok () body)`。
Result を返す関数は `(result (%inst $result T E))`。外側は宣言、内側は値型。

pair の状態を Result で返す例。match の `(result (%inst $result ...))` も外側が宣言、内側が値型である。
以下は明示形で、構築・分解する位置ごとに型を書く。型の alias と省略形も使える。

```wat
(module
  (%import "wlt:std" (type $pair) (type $result))
  (func $increment
    (param $state (%inst $pair i64 i32))
    (result (%inst $result (%inst $pair i64 i32) i32))
    (%effect pure)
    (%match (%inst $pair i64 i32) (result (%inst $result (%inst $pair i64 i32) i32))
      (local.get $state)
      (case pair ($id $count)
        (%result.ok (%inst $pair i64 i32) i32
          (%pair.new i64 i32 (local.get $id) (i32.add (local.get $count) (i32.const 1))))
      )))
  (%test "pair state"
    (%match (%inst $result (%inst $pair i64 i32) i32) (result i32)
      (call $increment (%pair.new i64 i32 (i64.const 7) (i32.const 41)))
      (case ok ($state)
        (%match (%inst $pair i64 i32) (result i32)
          (local.get $state)
          (case pair ($id $count) (i32.eq (local.get $count) (i32.const 42))))
      )
      (case err ($error) (i32.const 0))))
)
```

### result.let：成功時だけ続ける

```text
(%result.let ($value) (%inst $result T E) U subject body)
(%result.let ($value) subject body)    ;; 入力型と出力成功型を省略する形
```

subject は result<T,E>、body は result<U,E>、式全体も result<U,E>。
どちらの形も最後の2式が subject と成功時の body。型を省略しても評価順・スコープ・err の伝播は同じ。
引数の位置は次のとおり。**裸の U は出力の成功型だけ**で、入力型やエラー型ではない。

| 位置 | 指定するもの |
|---|---|
| `($value)` | subject の成功 payload を body 内だけで使う名前 |
| `(%inst $result T E)` | 入力 subject の値型 |
| `U` | body と式全体の成功型。エラー型 E は入力から引き継ぐ |
| `subject` / `body` | 入力の式／成功時だけ評価する式 |

T=unit なら `($value)` の代わりに `()`。初版の E は共有値。
err なら body を評価せず、同じ E の err をこの式の結果にする。**関数からの暗黙 return ではない。**

unit 入力の `()` は「成功payloadに名前を付けない」。成功時の継続bodyは省略しない。

```wat
(module
  (%import "wlt:std" (type $result) (type $unit))
  (func $after-unit
    (param $input (%inst $result $unit i32))
    (result (%inst $result i64 i32))
    (%effect pure)
    (%result.let ()
      (local.get $input)
      (%result.ok (i64.const 42))))
  (%test "unit success continues"
    (%match (call $after-unit (%result.ok $unit i32))
      (case ok ($n) (i64.eq (local.get $n) (i64.const 42)))
      (case err ($e) (i32.const 0))))
  (%test "unit error propagates"
    (%match (call $after-unit (%result.err $unit i32 (i32.const 7)))
      (case ok ($n) (i32.const 0))
      (case err ($e) (i32.eq (local.get $e) (i32.const 7))))
  ))
```

```wat
(module
  (%import "wlt:std" (type $result))
  (func $convert
    (param $input (%inst $result i64 i32))
    (result (%inst $result i32 i32))
    (%effect pure)
    (%result.let ($n) (%inst $result i64 i32) i32
      (local.get $input)
      (%result.ok i32 i32 (i32.wrap_i64 (local.get $n))))
  )
  (%test "result mapping"
    (%match (%inst $result i32 i32) (result i32)
      (call $convert (%result.ok i64 i32 (i64.const 42)))
      (case ok ($n) (i32.eq (local.get $n) (i32.const 42)))
      (case err ($e) (i32.const 0))))
)
```

## Future / Stream と終了処理

```text
(%future.ready T value)
(%future.await T (%move $future))       ;; future を消費し T を返す
(%future.close T (%move $future))
(%stream.empty T)
(%stream.cons T value rest-stream)    ;; rest-stream も消費する
(%stream.next T (%move $stream))       ;; pair<stream<T>, option<T>>
(%stream.close T (%move $stream))
```

await/next は常に suspends。EOF の next も端点を返すので、最後は close が必要。
payload は共有できる値に限定する。端点自身は共有 GC フィールドに保存しない。
通常の core 実行は配送済みデータ、Core＋JSPI / Component は Promise と外部 Stream にも接続する。

### stream.fold-result：失敗時も閉じる反復

```text
(%stream.fold-result T S E source initial ($state $item) body)
(%stream.fold-result source initial ($state $item) body)
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

task scope は言語のブロック構文ではなく JS ホスト側の実行単位。`task.cancel()` は要求後に解放完了を待つ Promise、`task.result` は取消し時に `TaskCancelledError` で reject する。取消しは待機点での協調的な処理で、CPUだけの処理の強制停止や既存の変更の巻き戻しは行わない。実行例は [タスクの取消し](docs/runtime.md#タスクの取消し) を参照。

## 型の alias と省略形

`(%type.alias $State (%inst $pair i64 i32))` は透明な型の略記。型の位置に `$State` と書く。
GC オブジェクトや nominal identity は増えない。`(ref $State)` のような heap 型名には使わない。
forward 参照可、循環は禁止。namespace 内の公開は `(%public (type $State))`、use も type と同じ。

関数の結果、引数、明示した let 型などから期待型が決まれば、値の構築に型引数を繰り返さない。
決まらない場合は型注釈を追加する。関数・if・block・明示形 match の結果宣言省略は従来どおり unit。

```wat
(module
  (%import "wlt:std" (type $pair) (type $result))
  (%type.alias $State (%inst $pair i64 i32))
  (func $increment
    (param $state $State)
    (result (%inst $result $State i32))
    (%let ($id $count) (local.get $state)
      (%result.ok
        (%pair.new (local.get $id) (i32.add (local.get $count) (i32.const 1))))
    ))
  (%test "short state"
    (%match (call $increment (%pair.new (i64.const 7) (i32.const 41)))
      (case ok ($state)
        (%let ($id $count) (local.get $state)
          (i32.eq (local.get $count) (i32.const 42))))
      (case err ($error) (i32.const 0))))
)
```

| 省略形 | 型の決め方 |
|---|---|
| `(%result.ok value)` / `(%result.err error)` | 期待する Result 型。unit payload の値引数は省略 |
| `(%pair.new first second)` | 期待する pair 型。unit 成分の値引数は省略 |
| `(%option.some value)` / `(%option.none)` / `(%own.new value)` | 期待する Option / Own 型 |
| `(%let ($first $second) subject body)` | subject の pair を一度だけ評価して分解。unit 成分の名前は省略 |
| `(%match subject (case ...)... )` | subject の型と期待する出力型。型付きの長い形も使用可 |
| `(%result.let ($value) subject body)` | subject の T/E と、期待型または body で局所的に決まる U |
| `(%stream.fold-result source initial ($state $item) body)` | source の T/E、initial または期待型の S。body は result<S,E> |

省略しても評価順、消費義務、effect、opaque の検査は同じ。result.let の E は共有かつ入力と出力で同じ型。fold の E も共有値、S は共有できる非 unit に限る。
pattern let で own 成分を含む pair を分解するときは `(%move $pair)` を使い、取り出した所有値も消費する。
任意の型の穴や関数をまたぐ全域型推論、暗黙の move / capture は導入しない。

## 所有値・クロージャ・effect

汎用関数・型・alias・クロージャは名前の直後に `(%type.params $T ...)` を宣言し、
`(%inst $name T...)` で具体化する。型引数は明示し、具体化後も型・effect・所有権を検査する。
具体的なクロージャ型も渡せる。[詳細と map/fold](docs/generics.md)。

```wat
(module
  (func $identity
    (%type.params $T)
    (%where (shared $T) (nonunit $T))
    (param $x $T)
    (result $T)
    (%effect pure)
    (local.get $x))

  (func $main
    (export "main")
    (result i64)
    (%effect pure)
    (call (%inst $identity i64) (i64.const 42))))
```

type.params の直後に `(%where (shared $T) (nonunit $T))` を置くと、具体化する型引数を制限できる。
共有は不変性を意味しない。コールバックには
`(callable $F (param $T) (result $U) (%effect pure))` と書ける。
引数・戻り値が一致する共有・再呼出し可能なクロージャに限定し、effect は本体から推論する。
effects を省略すれば effect の制限は課さない。未使用の適用も DCE 前に検査する。
適用は常に `(%inst $name T...)`。関数では `(call (%inst $identity i64) value)`、汎用型では `(ref (%inst $Box i64))`、汎用 alias では型の位置に `(%inst $Values i64)` と書く。
型引数の推論や型クラス・実装探索は行わない。型・effect・所有権は具体化した本体で検査し、未適用の汎用本体が任意の型引数で正しいという保証は与えない。

```text
(%own.new T value)
(%move $owner)                       ;; 所有値を消費する位置に渡す
(%own.close (%move $owner))            ;; 包みを消費。OS resource の destructor ではない

(%closure $Adder (%capture $base i64) (param $x i64) (result i64)
  (i64.add (local.get $base) (local.get $x)))
(%closure.new $Adder (i64.const 40))
(%closure.call $Adder closure (i64.const 2))
```

所有値に local.get、drop、move 後の再利用は不可。Option/Result/pair に所有成分があれば全体も所有値。
own は payload の GC 参照の別名を禁止しない。GC の回収と外部端点の終了処理は別。
closure は capture を全て宣言し、構築時に捕捉値を渡す。所有 capture の closure は1回だけ呼べる。

```wat
(module
  (%closure $Adder
    (%capture $base i64)
    (param $x i64)
    (result i64)
    (i64.add (local.get $base) (local.get $x)))

  (func $main
    (export "main")
    (result i64)
    (%let $add40 (%closure.new $Adder (i64.const 40))
      (%closure.call $Adder (local.get $add40) (i64.const 2))))
  (%test "explicit capture" (i64.eq (call $main) (i64.const 42))))
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
(%loop $go (bind $i i32 initial)... (result R) body)
(%recur $go next-value...)            ;; 最内側 loop の末尾だけ。同時更新
(%for $go (bind $i i32 initial)...
  (while condition) (step next-value...) unit-body)

(tag $Error (param i32))
(%try (result R)
  (do expression)
  (catch $Error ($code) handler)
  (catch_all handler))
(throw $Error payload...)
```

try/catch は Wasm 例外の糖衣。ゼロ除算・境界違反等の trap は catch_all で捕捉しない。
所有値を持つ関数内の try/throw は初版で制限されるため、その失敗には Result を使う。

## モジュール・不透明な公開型

```text
(%include "relative.wlt")
(%import "relative.wlt" (func $read $local) (type $State) (tag $Error))
(import "host" "read" (func $host-read (result i32)))
(export "main" (func $local))
(%namespace $stock
  (%export (opaque $State) (func $new) (func $get))
  (type $State (struct ...))
  (func $new ...)
  (func $get ...))
(%import $stock (type $State $Alias) (func $get))
(call $stock.get argument)
```

namespace 内は既定で非公開。`(%export (type $T))` は表現も公開し、`(%export (opaque $T))` は型だけ公開する。
opaque の構築・フィールド／配列操作・match は宣言 namespace 内だけ。別名経由も同じ。
include は宣言を挿入し、参照元ファイルからの相対パスを使う。親 namespace の名前は暗黙に取り込まず、完全修飾名はルートから。
`%public` と Wasm の `(export "main")` は別。ABI export はルートの関数に置く。
`%export` の名前リストは `%public` と、namespace からの `%import` は `%use` と同じ意味。
ファイル import は選んだ名前だけを束縛し、同じファイルの型・定義を共有する。import した別名も静的 export で再公開できる。
ルートの `(export "name" (func $local))` で import した関数を ABI export にできる。使わない定義は生成時に落ちる。
[境界・DCE・未対応範囲](docs/static-modules.md) を参照。

## bytes / text とライブラリ

| 式・関数 | 結果 |
|---|---|
| `(%bytes.new i32-expr...)` | 不変 bytes |
| `(%bytes.from-array $Array array)` | 非 null の i32 配列をコピー。各要素0..255以外は trap |
| `(%bytes.len b)` / `(%bytes.get b index)` | 長さ／0..255の要素。添字は符号なし、範囲外は trap |
| `(%bytes.slice b start length)` | コピーした bytes。範囲外は trap |
| `(%text.lit "日本語")` / `(%text.byte-len t)` | text／UTF-8 byte 長 |
| `(%text.bytes t)` / `(%text.from-bytes b)` | bytes／option<text>。不正 UTF-8 は none |
| `$string.concat(text,text)` | text の連結 |
| `$string.u64(i64)` / `$string.s64(i64)` | ビットパターンの符号なし／符号付き十進数化 |
| `$string.json(text)` | 引用符込みの JSON string |

標準ライブラリの利用入口は `wlt:std` に統一する。例えば `(%import "wlt:std" (type $text) (func $string.json))` で取り込み、通常の `(call $string.json ...)` で使う。内部の作業配列を扱う関数は reads/writes/allocates を持つ。
`stdlib/*.wlt` は同梱する実装のファイル。利用者の宣言群を挿入する `%include "program.wlt"` とは役割を分ける。`%include` は同じスコープへのテキスト挿入であり、暗黙に標準名を提供する仕組みではない。

検索・分割・整数解析には `$buffer.find-byte`、`$string.find`、`$string.split-once`、`$string.parse-u64` / `$string.parse-s64` を使う。
`parse-u64` は入力全体の ASCII 数字を検査し、符号なし64bitの範囲で some(i64) を返す。空・負数・`+`・空白・桁あふれは none。先頭ゼロは許す。成功値は i64 のビットパターンで、`string.u64` / `builder.u64` で符号なし十進数へ戻す。
位置は UTF-8 byte 単位。`$buffer.parse-u64(bytes,start,length,limit)` でアプリ固有の上限も指定でき、不正な入力は none になる。
[全 API と境界の規約](stdlib/README.md) を参照。JSON object はキー・値・区切りを自動でエスケープ／出力できる。

```wat
(module
  (%import "wlt:std" (type $text))
  (%import "wlt:std"
    (func $json.new)
    (func $json.string)
    (func $json.u64)
    (func $json.finish))
  (func $record
    (export "record")
    (param $name $text)
    (param $count i64)
    (result $text)
    (%effect reads writes allocates)
    (%let $o (call $json.new)
      (block
        (result $text)
        (call $json.string (local.get $o) (%text.lit "name") (local.get $name))
        (call $json.u64 (local.get $o) (%text.lit "count") (local.get $count))
        (call $json.finish (local.get $o))))
  ))
```

`$json.finish` は内容を返して空 object に戻す。text 断片を組む場合は `stdlib/builder.wlt` の `$builder.new` / `$builder.text` / `$builder.finish` を使う。

## 確認コマンド

`wlt-lang` を作業ディレクトリにする。実行には Wasm GC 対応 Wasmtime、JSPI の実行には Node 24以降を使う。

```sh
cargo run --locked -- check path/to/program.wlt
cargo run --locked -- types path/to/program.wlt
cargo run --locked -- expand path/to/program.wlt
cargo run --locked -- fmt path/to/program.wlt
cargo run --locked -- fmt-check path/to/program.wlt
cargo run --locked -- test path/to/tests.test.wlt
just build-stock-ledger
just stock-ledger
node runtime/build-core.mjs path/to/program.wlt target/my-program
node runtime/text-cli.mjs target/my-program --input path/to/input.tsv
```

最後の2行は任意の text I/O プログラムをビルドして入力ファイル付きで起動する例。
stdin を使う場合は `--input ...` を省略する。JSON 等の出力は stdout、main の Result が err なら診断と失敗終了になる。
text-cli は成功 payload を表示せず、ok の exit code は常に0。返された数値を確認するには既存の JS API を使う。
以下はプログラムの stdout を維持し、Result を stderr に表示する（`target/my-program` と入力パスを置き換える）。

```sh
node --input-type=module - target/my-program path/to/input.tsv <<'JS'
import { createReadStream } from 'node:fs';
import { runTextProgram } from './runtime/node-io.mjs';
const [, , program, input] = process.argv;
const result = await runTextProgram(program, {
  input: () => createReadStream(input), output: process.stdout,
});
console.error(JSON.stringify(result));
JS
```

例：行数を返す版なら `{"tag":"ok","val":3}`、I/O エラーなら `{"tag":"err","val":3}` 等を受け取る。
この観測用スクリプトは Result を表示するだけで、err をプロセスの失敗終了へ変換しない。

`(%test "name" i32-expr)` は0なら失敗、非0なら成功。`%law` は純粋な整数式の全入力を Lean で検査する別の契約。
実行用の stdin/stdout 接続は既存の text I/O capability。read-lines は stream<result<text,i32>>、write-line は future<result<unit,i32>> を返す。
暗黙の型引数推論・trait、任意のマクロ定義、一般的な borrow/lifetime、汎用の WASI binding は含まない。

## 数値・平坦な Result 連鎖・newtype・早期脱出

スカラーの変換・飽和変換・丸め・sqrt・min/max・reinterpret は標準 WAT の命令名をそのまま使う。
`i32.trunc_f64_s` は範囲外と NaN で trap、`i32.trunc_sat_f64_s` は飽和変換。

```text
(type $UserId (%newtype i64))
(%newtype.wrap $UserId scalar)        ;; 型検査後に消去。基底はi32/i64/f32/f64
(%newtype.unwrap $UserId id-value)    ;; 異なるnewtypeや数値への暗黙変換はない

(%result.do
  ($a first-result)
  ($b second-result)
  (() unit-success-result)
  final-result)

(return [value])
(block $done [(result R)] body...)
(br $done [value])
(br_if $done [value] condition)       ;; falseならvalueが残る
```

`%result.do` は1個以上の束縛と最後の Result 式。左から右に一度ずつ評価し、最初の err で残りを省略してその式の err を返す。エラー型はすべて同じ。`$a` は後の束縛から参照でき、unit 成功は `()` に束縛する。暗黙に関数から return しない。

```wat
(module
  (%import "wlt:std" (type $result))
  (type $Id (%newtype i32))
  (func $compute (result (%inst $result $Id i32)) (%effect pure)
    (%result.do
      ($a (%result.ok i32 i32 (i32.const 40)))
      ($b (%result.ok i32 i32 (i32.const 2)))
      (%result.ok $Id i32
        (%newtype.wrap $Id (i32.add (local.get $a) (local.get $b))))))
  (func $main (export "main") (result i32) (%effect pure)
    (%match (call $compute)
      (case ok ($id) (return (%newtype.unwrap $Id (local.get $id))))
      (case err ($code) (i32.const 0))))
  (%test "newtype and flat results" (i32.eq (call $main) (i32.const 42))))
```

名前付き block の分岐値と return 値は宣言した結果型に合わせる。`br_if` は値を条件より先に評価する。
`%using` を抜ける return・分岐・反復は、値の評価後に抜ける scope だけを逆順に close する。
所有値と try/catch を併用できるが、全経路で消費する。後続の引数が例外を投げて先の所有引数が取り残される場合も拒否する。
newtype は WLT 内で型を区別し、JavaScript/WIT 境界では基底の数値になる。値域検査や JavaScript の brand を自動では追加しない。
[詳しい制約と例](docs/core-expressions.md)。
