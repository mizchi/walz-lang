# WLTの基本の書き方

[導入](getting-started.md)を終えた人向けに、関数、値、データ構造、反復、モジュールを順に扱う。
以下の`wat`ブロックは完全なモジュールで、実行入口を持つ例の`main`は42を返す。
ファイル名の指定がなければ`main.wlt`へ保存し、`wlt check main.wlt`・`wlt run main.wlt`で試せる。
ファイルを分ける章だけは、ライブラリと実行入口の2ファイルを組み合わせる。
構文だけを探す場合は[チートシート](../CHEATSHEET.md)、対応範囲を調べる場合は[機能一覧](features.md)を使う。

## WATとWLT拡張を見分ける

`(命令 引数...)`というS式を使い、`;`を2個続けた行コメントや`$name`の識別子はWATと共通。
`func`、`call`、`if`、`struct.get`などはWasmの命令・宣言。
`%let`、`%match`、`%closure`など、WLTが追加する構文の入口には`%`が付く。
固定演算子`+`などと、型が決まる数値・ローカルの省略形は接頭辞のない糖衣として扱う。

```wat
(module
  (func $add (param $a i64) (param $b i64) (result i64)
    (%effect pure)
    (+ $a $b))
  (func $main (export "main") (result i64) (%effect pure)
    (call $add 40 2)))
```

`$a`は`(local.get $a)`、i64が期待される`40`は`(i64.const 40)`と同じ。
`(+ ...)`はここでは`i64.add`へ解決する。整数はWasmの幅で折り返し、除算のtrapもWasmに従う。
関数の引数・結果は宣言し、結果を省略した関数はunitを返す。

標準WAT単体はWasm validatorの経路で受け付ける。WLTの型・effect・所有権検査を受ける範囲は
型付きコアに限るため、任意のWAT全体に同じ静的保証が付くわけではない。[構文の境界](syntax.md)。

## 不変のローカルと期待型

`%let`は値を束縛し、最後の本体式の値を返す。複数の束縛をまとめると、初期値を左から右に評価する。

```wat
(module
  (func $main (export "main") (result i64) (%effect pure)
    (%let (($base (type i64) 40)
           ($delta (type i64) 2)
           ($answer (+ $base $delta)))
      $answer)))
```

後の初期値から先の束縛を読める。束縛は本体の中だけで有効で、再代入はできない。
単独なら`(%let $x (type i64) 42 $x)`とも書ける。
`(%let $x 42 ...)`のように整数幅の手掛かりがない初期値はエラーになる。
i32からi64への変換は`i64.extend_i32_s`などを明示する。[束縛](standard.md)・[数値と推論](core-expressions.md)。

## 条件式

条件はi32で、0が偽、非0が真。両方の枝が返す型をそろえる。

```wat
(module
  (func $choose (param $n i64) (result i64) (%effect pure)
    (if (result i64) (> $n 0)
      (then $n)
      (else 0)))
  (func $main (export "main") (result i64) (%effect pure)
    (call $choose 42)))
```

比較の`>`は整数の符号付き比較。浮動小数点や符号なしの比較には対応するWasm命令を使う。
`&&`・`||`は短絡評価し、結果をi32の0/1にする。[演算子](typeclasses.md)。

## GCデータと可変状態

構造体・配列・クロージャ環境はWasm GCが管理する。可変にしたいフィールドには`mut`を書く。
`wlt:core`の型・関数は使う名前を明示して取り込む。

```wat
(module
  (%use "wlt:core" (type $string))
  (type $Counter
    (struct (field $label $string) (field $value (mut i64))))
  (func $main (export "main") (result i64) (%effect reads writes allocates)
    (%let $counter (%$Counter::{:label "accepted" :value 40})
      (block (result i64)
        (%struct.update $Counter $counter ($value $old (+ $old 2)))
        (%let {:value $n} $counter $n)))))
```

`%$Counter::{...}`は型を指定した名前付き構築。全フィールドが必要で、型のないmap literalではない。
`%struct.update`は対象と旧値を一度読み、式の結果を書き戻す。原子的更新ではない。
GC参照を共有すると、その先の可変状態も共有する。`%let`の不変性は参照先を不変にはしない。
[名前付き構築](struct-construction.md)・[フィールド操作](high-level-forms.md)。

## enumと網羅的なmatch

独自の選択肢を`%variant`で宣言し、payloadの型と全caseを検査する。

```wat
(module
  (type $Decision (%variant (answer i64) (missing)))
  (func $main (export "main") (result i64) (%effect allocates)
    (%let $decision (%variant.new $Decision answer 42)
      (%match $decision
        (case answer ($n) $n)
        (case missing () 0)))))
```

payloadがなくても`()`を書く。caseの欠落・重複、payloadの型や個数の不一致はエラー。
通常のenumはGC参照、次のOption/Resultは内部表現を隠した値型になる。[チートシート](../CHEATSHEET.md)。

## OptionとResultで失敗を返す

値の欠如には`$option`、アプリケーションの失敗には`$result`を使う。
`(result ...)`はWATの戻り値宣言で、内側の`(%inst $result T E)`が成功型T・失敗型Eを持つ1値。

```wat
(module
  (%use "wlt:core" (type $result))
  (func $positive (param $n i64) (result (%inst $result i64 i32))
    (%effect pure)
    (if (result (%inst $result i64 i32)) (> $n 0)
      (then (%result.ok $n))
      (else (%result.err 1))))
  (func $main (export "main") (result i64) (%effect pure)
    (%let $value (call $positive 42)
      (%match $value
        (case ok ($n) $n)
        (case err ($code) 0)))))
```

Optionは`none`・`some`、Resultは`ok`・`err`を処理する。
成功値の連鎖は`%result.do`でも書ける。整数のゼロ除算などのtrapは自動でResultにならない。
Wasm例外はtagと`%try`で扱い、エラー値と使い分ける。[値操作](option-result.md)・[Resultの連鎖](core-expressions.md)。

## 反復は次の状態を返す

`%loop`の状態は各反復で不変。末尾の`%recur`に次の値を渡す。

```wat
(module
  (func $main (export "main") (result i64) (%effect pure)
    (%loop $sum (bind $i i32 0) (bind $total i64 0) (result i64)
      (if (result i64) (< $i 4)
        (then
          (%recur $sum (+ $i 1) (+ $total (i64.extend_i32_u $i))))
        (else (+ $total 36))))))
```

次の状態はすべて評価してから同時に更新する。再帰先は最内側のWLTループに限る。
終了条件・stepで書く場合は`%for`、共有可能なlistの各要素を処理する場合は`%for-each`を使える。
`%for-each`の本体はunit。集約には状態付きループや`$list.fold`を使う。
[ループの構文](../CHEATSHEET.md)・[list走査](high-level-forms.md)。

## 捕捉を明示したクロージャ

クロージャの型、捕捉する名前と型、呼び出しの引数・結果を宣言する。構築時に捕捉値を渡す。

```wat
(module
  (%closure $Adder (%capture $base i64)
    (param $n i64) (result i64) (%effect pure)
    (+ $base $n))
  (func $main (export "main") (result i64) (%effect allocates)
    (%let $add40 (%closure.new $Adder 40)
      (%closure.call $Adder $add40 2))))
```

呼び出し先はクロージャ型から決まる。暗黙に外側のローカルを捕捉しない。
可変状態はGCオブジェクトの参照を捕捉して共有する。所有値を捕捉するクロージャは1回だけ呼べる。
クロージャにも型引数を付けられる。[例](../examples/closure.wlt)・[ジェネリクス](generics.md)。

## 型引数と制約

`%type.params`で型パラメータを宣言し、`%where`で利用できる型を制限する。

```wat
(module
  (func $add (%type.params $T) (%where (Add $T))
    (param $a $T) (param $b $T) (result $T) (%effect pure)
    (+ $a $b))
  (func $main (export "main") (result i64) (%effect pure)
    (call $add 40 2)))
```

この呼び出しは結果の期待型からT=i64と決まる。明示するなら`(call (%inst $add i64) 40 2)`。
具体型ごとに単相化し、本体の型・effect・所有権を検査する。型・alias・クロージャの構築は型引数を明示する。
Addなどのクラスは組み込みで、利用者定義class/implや実行時の辞書はない。[ジェネリクス](generics.md)・[型クラス](typeclasses.md)。

## ファイルを分けて必要な名前を使う

次の2ファイルを同じディレクトリへ保存する。ファイルごとにスコープを持つ。

```wat
;; file: math.wlt
(module
  (%export (func $add))
  (func $add (param $a i64) (param $b i64) (result i64)
    (%effect pure) (+ $a $b)))
```

```wat
;; file: modules-main.wlt
(module
  (%use "./math.wlt" (func $add))
  (func $main (export "main") (result i64) (%effect pure)
    (call $add 40 2)))
```

`wlt run modules-main.wlt`で実行する。静的公開の`%export`、選択する`%use`、Wasmの`export`を分ける。
取り込まない名前は見えず、到達しない定義は検査後に除去する。未使用の不正な通常関数も検査対象。
ホスト関数には標準WATの`import`を使い、型・effect・capabilityをbinding policyで定める。
[静的モジュール](static-modules.md)・[WIT binding](wit-bindings.md)。

## 所有する値はmoveして終了する

Future・Stream・box・外部resourceなどの所有値は複製しない。消費する操作へ`%move`で渡す。

```wat
(module
  (%use "wlt:core" (type $future))
  (func $main (export "main") (result i64) (%effect suspends)
    (%let $pending (%future.ready i64 42)
      (%future.await i64 (%move $pending)))))
```

awaitはFutureを消費する。待たないFutureは`%future.close`で消費し、Streamは終端に到達しても最後にcloseする。
move後の利用や消費忘れは検査エラー。一般的なRustの参照lifetimeや、GCオブジェクトの排他的所有を導入するものではない。
GCによる到達性の回収と、ホストの外部資源を閉じる操作は区別する。

この例は配送済みの値なのでCoreでも実行できる。外部Promiseを待つ場合はCore＋JSPIやComponentのホストを使う。
子タスクのspawn/joinは同一インスタンス内の協調並行で、Workerによる並列処理とは別。
[ランタイム](runtime.md)・[所有権](verification.md)・[子タスク](scoped-tasks.md)。

## 次に読むもの

string・bytes・list・dict・view・JSONの操作は[標準API一覧](stdlib-api.md)から探す。
繰り返し使う値計算は通常関数へまとめ、名前の束縛や構文を拡張したい場合に[衛生的マクロ](user-macros.md)を使う。
実用的な構成は[ログ集計](../examples/apps/log-stats/README.md)・[在庫台帳](../examples/apps/stock-ledger/README.md)・
[ワークフロープランナー](../examples/apps/workflow/README.md)を参照。
JS/TSから呼び出す場合は[Vite](vite.md)と[Content Mapper](content-mapper.md)へ進む。
