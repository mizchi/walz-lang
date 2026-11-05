# gcwat 文法チートシート

標準 WAT を基底に、型付きの式・GC データ・所有値・固定マクロを追加した言語。
ここでは拡張を使う型付きコアを扱う。標準 WAT 単体は Wasm validator に通るものを受理するが、任意の WAT 命令と拡張の混在はまだできない。
詳しい意味論は [SPEC.md](SPEC.md)、実用例は [在庫台帳](examples/apps/stock-ledger/README.md)。

## 最初に覚える規則

- プログラムは `(module 宣言...)`。関数の本体は1つの式。複数の処理には block を使う。
- `$name` は識別子、`"text"` は文字列、`;;` は行コメント、`(; ... ;)` は入れ子可能なコメント。
- 引数と演算は左から右へ評価する。暗黙の数値変換・捕捉・ローカル再代入はない。
- **`(result R)` は結果型の宣言。`(result T E)` は成功 T／エラー E の値型。**
- 結果宣言を省略すると unit。本体から結果型を推論する指定ではない。
- 共有 GC 参照と、消費が必要な own / Future / Stream を区別する。

以下の `T U E S`、`expr`、`body`、`...`、`[省略可能]` は説明用の記号。字面をソースに書くものではない。
`wat` のコードブロックはそれぞれコンパイルできる完全なモジュール、`text` は構文のひな形。

## 関数・型・束縛

```wat
(module
  (func $add (param $a i64) (param $b i64) (result i64) (effects pure)
    (i64.add (local.get $a) (local.get $b)))
  (func $main (export "main") (result i64) (effects pure)
    (let $answer (call $add (i64.const 40) (i64.const 2))
      (local.get $answer)))
  (test "answer" (i64.eq (call $main) (i64.const 42))))
```

| 型 | 意味 |
|---|---|
| `i32 i64 f32 f64` | Wasm の数値型。整数の加減乗算は折り返し、整数のゼロ除算等は trap。符号解釈は命令の `_s` / `_u` で指定 |
| `(ref $T)` / `(ref null $T)` | GC の非 null／nullable 参照。型は nominal |
| `bytes` / `text` | 不変なバイト列／妥当な UTF-8。内部参照の操作や比較はできない |
| `(option T)` / `(result T E)` | none/some、ok/err の値型 |
| `(pair T U)` | 2成分の値。成分が共有値なら pair も共有値 |
| `(own T)` / `(future T)` / `(stream T)` | 消費する所有値。GC 参照への排他的所有を意味しない |
| `unit` | 値のない結果。関数の引数・通常のフィールドには格納しない |

```text
(func $f (param $x T)... [(result R)] [(effects ...)] body)
(let $x [T] initializer body)        ;; T は省略時に推論。不変の字句束縛
(local.get $x)
(call $f arg...)
(block [(result R)] unit-expr... final-expr)
(if [(result R)] i32-condition (then expr) (else expr))
(drop shared-value)                 ;; unit の式へ。所有値は drop しない
(nop)                              ;; unit
(unreachable)                      ;; trap。Wasm 例外とは異なる
```

if / block / match の結果宣言がない場合も unit。最後以外の block の式は unit にする。
一時的な整数を捨てる場合は `(drop ...)`、状態の更新は mutable な GC フィールド／配列へ明示する。

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

```wat
(module
  (type $Item (variant (none) (count i64)))
  (func $get (param $item (ref $Item)) (result i64)
    (match $Item (result i64) (local.get $item)
      (case none () (i64.const 0))
      (case count ($n) (local.get $n))))
  (test "variant" (i64.eq (call $get (variant.new $Item count (i64.const 42))) (i64.const 42))))
```

全ケースを1回ずつ書く。payload のない case も `()` が必要。型・個数・網羅性を検査する。
通常の enum は `(ref $Item)`、Option/Result/pair は参照を直接操作しない値型。

## Option / Result / pair

```text
(option.none T)
(option.some T value)
(result.ok T E value)
(result.err T E error)
(pair.new T U first second)

(match (option T) (result R) subject
  (case none () no-value) (case some ($value) with-value))
(match (result T E) (result R) subject
  (case ok ($value) success) (case err ($error) failure))
(match (pair T U) (result R) subject
  (case pair ($first $second) body))
```

unit の成分は値引数と payload 束縛を省略する。例：`(result.ok unit i32)`、`(case ok () body)`。
Result を返す関数は `(result (result T E))`。外側は宣言、内側は値型。

pair の状態を Result で返す例。match の `(result (result ...))` も外側が宣言、内側が値型である。
以下は明示形で、構築・分解する位置ごとに型を書く。型の alias と省略形も使える。

```wat
(module
  (func $increment (param $state (pair i64 i32))
    (result (result (pair i64 i32) i32)) (effects pure)
    (match (pair i64 i32) (result (result (pair i64 i32) i32)) (local.get $state)
      (case pair ($id $count)
        (result.ok (pair i64 i32) i32
          (pair.new i64 i32 (local.get $id) (i32.add (local.get $count) (i32.const 1)))))))
  (test "pair state"
    (match (result (pair i64 i32) i32) (result i32)
      (call $increment (pair.new i64 i32 (i64.const 7) (i32.const 41)))
      (case ok ($state)
        (match (pair i64 i32) (result i32) (local.get $state)
          (case pair ($id $count) (i32.eq (local.get $count) (i32.const 42)))))
      (case err ($error) (i32.const 0)))))
```

### result.let：成功時だけ続ける

```text
(result.let ($value) (result T E) U subject body)
```

subject は result<T,E>、body は result<U,E>、式全体も result<U,E>。
引数の位置は次のとおり。**裸の U は出力の成功型だけ**で、入力型やエラー型ではない。

| 位置 | 指定するもの |
|---|---|
| `($value)` | subject の成功 payload を body 内だけで使う名前 |
| `(result T E)` | 入力 subject の値型 |
| `U` | body と式全体の成功型。エラー型 E は入力から引き継ぐ |
| `subject` / `body` | 入力の式／成功時だけ評価する式 |

T=unit なら `($value)` の代わりに `()`。初版の E は共有値。
err なら body を評価せず、同じ E の err をこの式の結果にする。**関数からの暗黙 return ではない。**

```wat
(module
  (func $convert (param $input (result i64 i32)) (result (result i32 i32)) (effects pure)
    (result.let ($n) (result i64 i32) i32 (local.get $input)
      (result.ok i32 i32 (i32.wrap_i64 (local.get $n)))))
  (test "result mapping"
    (match (result i32 i32) (result i32) (call $convert (result.ok i64 i32 (i64.const 42)))
      (case ok ($n) (i32.eq (local.get $n) (i32.const 42))) (case err ($e) (i32.const 0)))))
```

## Future / Stream と終了処理

```text
(future.ready T value)
(future.await T (move $future))       ;; future を消費し T を返す
(future.close T (move $future))
(stream.empty T)
(stream.cons T value rest-stream)    ;; rest-stream も消費する
(stream.next T (move $stream))       ;; pair<stream<T>, option<T>>
(stream.close T (move $stream))
```

await/next は常に suspends。EOF の next も端点を返すので、最後は close が必要。
payload は共有できる値に限定する。端点自身は共有 GC フィールドに保存しない。
通常の core 実行は配送済みデータ、Core＋JSPI / Component は Promise と外部 Stream にも接続する。

### stream.fold-result：失敗時も閉じる反復

```text
(stream.fold-result T S E source initial ($state $item) body)
```

source は stream<result<T,E>>、initial は S、body は result<S,E>。全体は result<S,E>。
source、initial の順に一度だけ評価する。S は共有できる非 unit の状態。
EOF は close して ok(state)、入力の err／body の err は close して err(E)。
T=unit の束縛は `($state)` だけ。body は同じ関数の loop/match へ展開し、closure は生成しない。
任意の throw に対する finally ではない。取消しの終了確認は既存の task scope が担当する。

## 型の alias と省略形

`(type.alias $State (pair i64 i32))` は透明な型の略記。型の位置に `$State` と書く。
GC オブジェクトや nominal identity は増えない。`(ref $State)` のような heap 型名には使わない。
forward 参照可、循環は禁止。namespace 内の公開は `(public (type $State))`、use も type と同じ。

関数の結果、引数、明示した let 型などから期待型が決まれば、値の構築に型引数を繰り返さない。
決まらない場合は型注釈を追加する。関数・if・block・明示形 match の結果宣言省略は従来どおり unit。

```wat
(module
  (type.alias $State (pair i64 i32))
  (func $increment (param $state $State) (result (result $State i32))
    (let ($id $count) (local.get $state)
      (result.ok (pair.new (local.get $id) (i32.add (local.get $count) (i32.const 1))))))
  (test "short state"
    (match (call $increment (pair.new (i64.const 7) (i32.const 41)))
      (case ok ($state)
        (let ($id $count) (local.get $state) (i32.eq (local.get $count) (i32.const 42))))
      (case err ($error) (i32.const 0)))))
```

| 省略形 | 型の決め方 |
|---|---|
| `(result.ok value)` / `(result.err error)` | 期待する Result 型。unit payload の値引数は省略 |
| `(pair.new first second)` | 期待する pair 型。unit 成分の値引数は省略 |
| `(option.some value)` / `(option.none)` / `(own.new value)` | 期待する Option / Own 型 |
| `(let ($first $second) subject body)` | subject の pair を一度だけ評価して分解。unit 成分の名前は省略 |
| `(match subject (case ...)... )` | subject の型と期待する出力型。型付きの長い形も使用可 |
| `(result.let ($value) subject body)` | subject の T/E と、期待型または body で局所的に決まる U |
| `(stream.fold-result source initial ($state $item) body)` | source の T/E、initial または期待型の S。body は result<S,E> |

省略しても評価順、消費義務、effect、opaque の検査は同じ。result.let の E は共有かつ入力と出力で同じ型。fold の E も共有値、S は共有できる非 unit に限る。
pattern let で own 成分を含む pair を分解するときは `(move $pair)` を使い、取り出した所有値も消費する。
任意の型の穴や関数をまたぐ全域型推論、暗黙の move / capture は導入しない。

## 所有値・クロージャ・effect

```text
(own.new T value)
(move $owner)                       ;; 所有値を消費する位置に渡す
(own.close (move $owner))            ;; 包みを消費。OS resource の destructor ではない

(closure $Adder (capture $base i64) (param $x i64) (result i64)
  (i64.add (local.get $base) (local.get $x)))
(closure.new $Adder (i64.const 40))
(closure.call $Adder closure (i64.const 2))
```

所有値に local.get、drop、move 後の再利用は不可。Option/Result/pair に所有成分があれば全体も所有値。
own は payload の GC 参照の別名を禁止しない。GC の回収と外部端点の終了処理は別。
closure は capture を全て宣言し、構築時に捕捉値を渡す。所有 capture の closure は1回だけ呼べる。

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
(loop $go (bind $i i32 initial)... (result R) body)
(recur $go next-value...)            ;; 最内側 loop の末尾だけ。同時更新
(for $go (bind $i i32 initial)...
  (while condition) (step next-value...) unit-body)

(tag $Error (param i32))
(try (result R)
  (do expression)
  (catch $Error ($code) handler)
  (catch_all handler))
(throw $Error payload...)
```

try/catch は Wasm 例外の糖衣。ゼロ除算・境界違反等の trap は catch_all で捕捉しない。
所有値を持つ関数内の try/throw は初版で制限されるため、その失敗には Result を使う。

## モジュール・不透明な公開型

```text
(include "relative.gwat")
(namespace $stock
  (public (opaque $State) (func $new) (func $get))
  (type $State (struct ...))
  (func $new ...)
  (func $get ...))
(use $stock (type $State $Alias) (func $get))
(call $stock.get argument)
```

namespace 内は既定で非公開。`public (type $T)` は表現も公開し、`public (opaque $T)` は型だけ公開する。
opaque の構築・フィールド／配列操作・match は宣言 namespace 内だけ。別名経由も同じ。
include は宣言を挿入し、参照元ファイルからの相対パスを使う。親 namespace の名前は暗黙に取り込まず、完全修飾名はルートから。
`public` と Wasm の `(export "main")` は別。ABI export はルートの関数に置く。

## bytes / text とライブラリ

| 式・関数 | 結果 |
|---|---|
| `(bytes.new i32-expr...)` | 不変 bytes |
| `(bytes.from-array $Array array)` | 非 null の i32 配列をコピー。各要素0..255以外は trap |
| `(bytes.len b)` / `(bytes.get b index)` | 長さ／0..255の要素。添字は符号なし、範囲外は trap |
| `(bytes.slice b start length)` | コピーした bytes。範囲外は trap |
| `(text.lit "日本語")` / `(text.byte-len t)` | text／UTF-8 byte 長 |
| `(text.bytes t)` / `(text.from-bytes b)` | bytes／option<text>。不正 UTF-8 は none |
| `$string.concat(text,text)` | text の連結 |
| `$string.u64(i64)` / `$string.s64(i64)` | ビットパターンの符号なし／符号付き十進数化 |
| `$string.json(text)` | 引用符込みの JSON string |

ライブラリはルート宣言位置で、呼出し元からの相対パスで `stdlib/text.gwat` を include する。
関数の使用は通常の `(call $string.json ...)`。内部の作業配列を扱う関数は reads/writes/allocates を持つ。

## 確認コマンド

`examples/gcwat` を作業ディレクトリにする。実行には Wasm GC 対応 Wasmtime、JSPI の実行には Node 24以降を使う。

```sh
cargo run --locked -- check path/to/program.gwat
cargo run --locked -- types path/to/program.gwat
cargo run --locked -- expand path/to/program.gwat
cargo run --locked -- fmt path/to/program.gwat
cargo run --locked -- fmt-check path/to/program.gwat
cargo run --locked -- test path/to/tests.test.gwat
just build-stock-ledger
just stock-ledger
node runtime/build-core.mjs path/to/program.gwat target/my-program
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

`test "name" i32-expr` は0なら失敗、非0なら成功。`law` は純粋な整数式の全入力を Lean で検査する別の契約。
実行用の stdin/stdout 接続は既存の text I/O capability。read-lines は stream<result<text,i32>>、write-line は future<result<unit,i32>> を返す。
単相化ジェネリクス、任意のマクロ定義、borrow/lifetime、汎用の WASI binding は初版に含まない。
