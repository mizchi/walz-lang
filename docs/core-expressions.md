# 数値・Result の連鎖・newtype・早期脱出

いずれも型付きコアで検査する。標準 WAT の数値命令、`return`、`block`、`br`、`br_if` に `%` は付けない。追加する糖衣と型操作には付ける。

## スカラー数値命令

`i32` / `i64` の回転・符号拡張、浮動小数点と整数の変換、`f32` / `f64` の `abs`・`ceil`・`floor`・`trunc`・`nearest`・`sqrt`・`min`・`max`・`copysign`、`reinterpret`、`demote` / `promote` を直接扱う。命令の署名と trap の分類は `src/ir.rs` の `numeric_ops!` に集約する。

`(i32.trunc_f64_s (f64.sqrt (f64.const 1764)))` は42になる。変換に暗黙の丸め規則は追加しない。`trunc_*` は NaN や範囲外で Wasm の trap、`trunc_sat_*` は飽和変換になる。`nearest` は Wasm の偶数丸め。semantic effect の `pure` は trap しないという意味ではない。整数 law の証明対象も自動では拡大しない。

## ローカル参照と数値の省略形

式の位置にある`$foo`は`(local.get $foo)`と同じ。引数・let・ループ状態・case・catch・明示的なクロージャ捕捉の束縛を参照できる。フィールド名は束縛を作らず、globalや関数を自動で参照することもない。所有値には従来どおり`(%move $foo)`が必要。

```wat
(module
  (func $inc (param $foo i64) (result i64)
    (i64.add $foo 1))
  (func $main (export "main") (result i64)
    (%let (($start (type i64) 41)
           ($answer (call $inc $start)))
      $answer)))
```

letの型注釈は`(type T)`で表せる。単一の`(%let $x (type i32) 1 body)`と複数束縛のどちらでも使え、Tにはalias・参照型・`(%inst ...)`も置ける。注釈のない`(%let $x $foo body)`は、既存の`$foo`の型を使う。

裸の数値は、期待型が`i32`・`i64`・`f32`・`f64`のいずれかに確定する式位置で、その型の`.const`へ展開する。期待型は、型注釈、関数の引数・戻り値、数値/SIMD命令のオペランド、宣言したblock/if/matchの結果、struct/arrayの要素、型を指定した値構築などから与える。条件式と配列インデックスはi32。`(i64.eq 1 2)`はi64の定数を比較してi32を返す。

`(%let $x 1 body)`や`(drop 1)`は期待型がないためエラーになる。数値の大きさから幅を選ばず、後続の使用箇所から型を逆算しない。`(result)`の省略はunitのままであり、本体が裸の数値でも戻り値型を推論しない。
範囲・16進数・浮動小数点・NaN/InfはWATの定数規則に従う。明示した`(i32.const 1)`やi32ローカルをi64へ暗黙に変換せず、newtypeを自動でwrapしない。v128には明示的な`v128.const`を使う。

文字列の省略は[名前付きstructの初期値](struct-construction.md)に限る。型付きコアの外にあるWATのスタック構文や即値は、この省略形の対象にしない。標準WATとして有効な入力は引き続きそのままコンパイルする。
展開後は既存のLocal/Constを使い、型・effect・所有権・領域の検査、最適化、`type-at`に同じ情報を渡す。実行例は[expression-sugar.wlt](../examples/expression-sugar.wlt)、検査は`just test-expression-sugar`。

基本型には`+`・`=`・`<`等の[型クラス演算子](typeclasses.md)もある。型が確定すると、ここで扱う既存の数値命令へ直接展開する。

## `%result.do`

```wat
(%result.do
  ($a (call $first))
  ($b (call $second (local.get $a)))
  (() (call $check (local.get $b)))
  (%result.ok (local.get $b)))
```

1個以上の束縛と最後の Result 式を置く。各束縛は `($name Result式)`。成功 payload が unit の場合は `(() Result式)` とする。左から右へ一度ずつ評価し、成功値の名前は後の束縛と最後の式で使える。最初の err で残りを評価せず、その err を **この式の結果** にする。関数から暗黙には return しない。

既存の `%result.let` の入れ子へ展開する。成功型は段階ごとに変えられるが、エラー型はすべて同一で共有可能な型にする。所有値の成功 payload は明示的に move する。err の経路にも未消費の所有値を残せない。

## スカラー newtype

```wat
(type $UserId (%newtype i64))
(%newtype.wrap $UserId (i64.const 42))
(%newtype.unwrap $UserId id-expression)
```

基底は `i32` / `i64` / `f32` / `f64` のいずれか。型の位置では `$UserId` と書く。同じ基底の別の newtype、基底の数値型とは区別し、変換を明示する。演算には unwrap し、必要なら結果を wrap する。newtype は shared/nonunit を満たし、GC フィールド、enum、標準の値型、単相化の型引数に使える。

型・effect・所有権・opaque を検査した後に消去する。wrap/unwrap のための GC オブジェクトや実行時の型タグを作らない。`types` / `type-at` では nominal な型名を保持する。`opaque` で公開した型の wrap/unwrap は宣言した namespace 内だけで使える。

Core＋JSPI と Component の境界では基底のスカラーとして渡す。list や Future の payload でも同じ。JavaScript/WIT の値に型名や brand は残らず、外部入力の値域制約を保証する仕組みではない。値域の検査は公開する生成関数で別に行う。GC 参照や v128 を基底とする newtype、暗黙の演算委譲は提供しない。

## return とラベル付き分岐

```wat
(block $done (result i32)
  (drop (br_if $done (i32.const 42) condition))
  (return (i32.const 7)))
```

`return` の値は関数の結果型に合わせる。unit 関数なら `(return)`。`br` / `br_if` は名前で囲んでいる block または loop を指定し、block の結果、loop の入力 state に合わせる。`br_if` は値を先に、条件を後に評価し、非0なら分岐する。分岐しなければ値が式の結果として残る。上の例では false 側で42を drop してから7を return する。

型付きコアの結果は unit または1値。数値ラベル、複数結果、`br_table` などを使う標準 WAT は従来の WAT 検証経路で扱う。`%recur` は引き続き最内側の WLT loop の末尾位置に限る。`%for` も通常のラベルへ展開する。非ローカルな例外や専用の break/continue は追加しない。

`%using` の外へ return・分岐・recur するときは、値と条件を評価してから、抜ける scope の資源を内側から逆順に close する。内側の block を抜けるだけなら外側の資源は閉じない。destructor の呼出し前に解放フラグを落とし、例外の unwind と重なっても二度呼ばない。borrow を分岐の payload にすることは禁止する。

## 例外経路の所有権

所有値を持ったまま `%try` と例外を投げる呼出しを使える。各 catch 入口、通常の合流点、loop 入口に到達する所有集合が一致することを検査する。各終了経路で move・返却・分解・close を完了する。handler 自身の throw は外側へ伝播する。

引数などの評価途中の所有値も追跡する。`(call $consume (%move $x) (call $may-throw))` で後の引数が例外を投げると、先の所有値が取り残される。そのようなプログラムは拒否する。例外を投げる計算を先に束縛するか、取得・move する前に失敗を処理する。

`contracts` は新しい制御がある関数について version 2 の制御フロー証明書を出す。各ノードの利用可能な所有 slot と遷移を、Lean が独立に検査する。version 1 の既存契約も受け付ける。Lean の定理は各遷移と任意回の反復を含む経路上の所有状態を扱う。IR からの抽出、effect 推論、Wasm 出力、borrow の非流出、ホスト資源の registry は TCB に残る。

所有する例外 payload、所有値を含む Future/Stream payload、部分 move、一般的な lifetime は追加しない。Wasm trap は catch の対象ではなく、外部資源はホスト registry の終了処理も使う。

`just test-core-expressions` で数値・Result・newtype・制御・例外所有権と両 ABI を、`just test-e2e` で Chromium を検査できる。`examples/resources/main.wlt` に早期脱出と解放の例、`tests/component-fixtures/language-core.wlt` に組合せの例がある。
