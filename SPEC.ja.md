# walz v0 — WAT の基底と型付き拡張

[English](SPEC.md) | [日本語](SPEC.ja.md)

## 標準 WAT との互換性

標準 WAT として parse・validate できる入力を優先し、ソースを変更せず Wasm にします。
対応範囲は固定した `wat`/`wasmparser` の既定の検証機能です。スタック形式、匿名関数、数値インデックス、線形メモリ、バイト文字列、標準の例外命令などを型付きコアの制限で拒否しません。
元の WAT にローカルの再代入や動的な呼び出しがあれば、その Wasm の意味を保ちます。`$__` の予約も拡張側だけの規約です。

Walz の拡張入口はすべて `@` で始めます。標準の `import` / `export` と、静的リンクの `@use` / `@export` を区別します。値型の `(@inst $result T E)` と結果宣言の `(result T)` も区別します。型引数は `(@inst $name T...)` で適用します。[接頭辞と解析の境界](docs/syntax.md) に規約をまとめています。

既知の Walz `@` 名は予約され、必ず検査します。それ以外の WAT 注釈とその入れ子の内容は opaque として扱い、メタデータ内の拡張に見える形式は実行しません。WIT の識別子エスケープと URI のパーセント符号化には引き続き `%` を使います。
式のローカル・数値の省略形と、固定の記号演算子には接頭辞を付けません。算術・比較に加え、`&&`・`||`・`&`・`|`・`^`・`<<`・`>>`・`>>>`を提供します。

追加構文は、以下の型付きコアで名前・型・束縛・網羅性を検査し、標準 WAT へ展開します。このコアの本体は folded expression 規約に従います。コア外の WAT と構文マクロ `@let` / `@loop` / `@for` を併用する場合は、展開後の Wasm 全体を検証し、解析を opaque とします。この経路から effect・所有権などの契約を検査済みと扱うことはありません。
組み込み構文の展開規則は固定します。利用者の構文拡張は宣言的な式マクロで定義でき、eval や任意のコンパイル時プログラムは実行しません。
`expand`/`wat` コマンドと `walz::expand` で展開結果を確認できます。

ユーザー定義の式構文は `@macro` で宣言し、`@$name` で呼び出します。宣言的な pattern/template、構文カテゴリ、衛生的な束縛、明示的な `bind` スコープ、通常の型検査器への `infer` 問い合わせと `when` 制約を提供します。生成物は既存の型付きWALZへ戻し、単相化・型・effect・所有権・race検査を通します。新しい型規則や検査済みIRをマクロから登録するAPIはありません。原文位置と展開コンテキストを保持し、LSPも宣言された束縛規則を利用します。

反復する ident にも `bind` を宣言でき、同じ束縛関係を宣言したマクロへ本体を丸ごと委譲できます。捕捉した初期化式は呼び出し元のスコープで解釈し、新しい束縛が見えるのは宣言した本体だけです。通常の展開後は影響する式から検査を再開し、検査済みの外側や先行する式を保持します。推論の試行で作った一時ローカルは持ち越さず、新しい具体化で型環境が変われば全体を検査し直します。関数完成時の構文検査と最終的な全体の契約検査も行います。構文・上限・公開APIは [ユーザー定義マクロ](docs/user-macros.md) を参照してください。

`walz::validate` と CLI の `check` は両経路に対応します。`walz::check` は詳細な型を得るための型付きコア API です。
`compile` の `ir` は `Option<Module>` とし、標準 WAT がコアの外にある場合は `None` です。
その場合も Wasm の静的検証を行い、`native::types` と `types` コマンドで関数の引数・複数の戻り値・export を取得できます。式ごとの位置情報は提供しません。
詳細な解析ができない入力は `analysis.opaque=true` と `module_effects` の全項目 true を返します。空の関数一覧を「作用なし」と解釈してはいけません。
型付きコアの `module_effects` は全関数の作用の和であり、モジュールのインスタンス化そのものの作用を表すものではありません。

以下の型・不変性・構文の規則は型付き拡張のものです。

## 表現と型

Walz の基本データ型は `(@use "walz:core" (type $result) (type $option) (type $future) ...)` で明示的に取り込みます。型の適用は `(@inst $result T E)`、`(@inst $future T)` のように、利用者定義の汎用型と同じ構文です。`$string`・`$bytes`・`$unit` は型引数なしの alias です。Wasm の数値型や `ref` には import を必要としません。[標準モジュール](docs/standard.md) を参照してください。

プログラムは1つの `(module ...)`。宣言は WAT 形式の `type`・`func`・関数 `import`・`tag` と、追加の `@closure`・`@test`・`@bench`・`@law`および静的な named `@use` / `@export` です。ファイルの宣言位置では `@include` も使えます。
型は scalar newtype の `$Name`・`i32`・`i64`・`f32`・`f64`・`v128`・`(ref $T)`・`(ref null $T)`・`(@inst $option T)`・`(@inst $result T E)`・`(@inst $tuple T U)`・`(@inst $box T)`・`(@inst $future T)`・`(@inst $stream T)`・`$bytes`・`$string`。戻り値を宣言しない式・関数は unit です。

`v128` と通常 SIMD の非メモリ命令は型付きコアで扱います。lane の形は命令で指定し、型は共通の `v128` です。即値とオペランドを検査し、Wasm 命令へ直接出力します。ベクトル自体は shared/nonunit を満たす値で、GC格納・捕捉・単相化にも使えます。対応範囲と使用例は [SIMD](docs/simd.md) を参照してください。
関数の引数・戻り値、構造体のフィールド、配列の要素、クロージャの捕捉値には型が必要です。
`@let` の束縛型は注釈がなければ初期値から一意に決まり、`case` の束縛型は payload の宣言から決まります。
`(@let $x (type i32) 1 body)`の`(type T)`は明示的なlet注釈です。複数束縛でも使えます。式位置の`$foo`は`(local.get $foo)`であり、裸の数値は期待するスカラー型の`.const`に展開します。期待型のない数値には既定幅を設けず、型エラーとします。所有値のmove・クロージャ捕捉・newtypeのwrapは明示します。[省略式の規約](docs/core-expressions.md#ローカル参照と数値の省略形)を参照してください。
暗黙の型変換はなく、non-null 参照から同じ型の nullable 参照への適合だけを認めます。
GC 型は名前で区別します。前方参照と再帰型を認め、出力時には1つの Wasm recursion group にまとめます。

WAT と同じ数値意味論を使います。整数演算はビット幅で折り返し、符号付き・符号なしの解釈は命令名で区別します。
整数の除算・剰余は Wasm の条件で trap します。浮動小数点も Wasm の意味に従います。
スカラーの変換・飽和変換・丸め・sqrt・min/max・reinterpret も型付きコアで直接扱います。[数値と制御の規約](docs/core-expressions.md) に例があります。
型付きコアで扱う数値命令の一覧と型制約は `src/ir.rs` の `numeric_ops!` が定義します。標準 WAT 経路の命令集合はこの表に限定しません。

名前解決・静的型検査・effect 契約・所有権の検査は `walz::check(&str)` で独立して実行でき、検査済みの型付き IR を返します。law の証明はこの API では実行しません。
関数の引数・戻り値、呼び出し、数値命令、GC データの読み書き・構築、enum の分解、クロージャの捕捉、ループの状態更新をこの段階で検査します。
型推論は宣言済みの型に基づく単相の推論です。組込みの値型構築子と、明示した利用者定義の汎用関数・汎用型は型引数ごとに具体化します。通常の汎用関数の `call` は引数・期待型・既知の callable シグネチャから型引数が一意に決まる場合に省略できます。関数シグネチャの全域推論や暗黙の数値型変換は行いません。[単相化の契約](docs/generics.md) を参照してください。
戻り値注釈を省略した関数や result を省略した block/if/match の型は unit であり、本体から戻り値型を推論する指定ではありません。

`typing::report(&module)` は、関数ごとの params/captures の ID、戻り値型、全束縛の ID・名前・宣言位置・型・束縛の種類、全ソース式の位置・型を返します。
同名の束縛も関数内の ID で区別します。型の報告では alias を展開した基底表記を使い、`i64`・`(ref $T)`・`(ref null $T)`・`@unit`・`never` のように表示します。ソースの標準型は `walz:core` から import した名前で参照します。
位置は API に渡したソース（include を使う場合は展開後）の UTF-8 byte offset の半開区間 `[start, end)` です。省略された unit 本体にはソース式の位置がなく、式の一覧には含めません。
`typing::type_at(&module, offset)` で束縛の宣言名または最も内側の式の型を取得でき、対象範囲外は `None` です。
型の問い合わせは既に解決した型を参照し、Wasm を実行しません。

## 束縛と式

識別子は `$` と ASCII 英数字・`_`・`.`・`-`。`$__` はコンパイラ用に予約します。
束縛は字句スコープで、内側の同名束縛による隠蔽を認めます。引数・捕捉宣言の重複はエラーです。

```text
(@let $name [(type T)] initial-value body)
(@let (($first [(type T)] initial-first) ($second [(type U)] initial-second) ...) body)
(@let {:field $binding ...} struct-value body)
(local.get $name)
(block [(result T)] unit-expression ... result-expression)
(if [(result T)] condition (then expression) (else expression))
(call $function argument ...)
(drop value)
(nop)
(unreachable)
```

式は folded expression、またはローカル参照・期待型付き数値の省略形とし、1関数の本体は1つの式です。複数の処理は `block` でまとめます。
block の最後以外の式は unit でなければならず、不要な値は `drop` で明示します。
条件は `i32`、分岐の型は result と一致させます。`unreachable` と `@recur` は値を返さず、任意の結果型の位置で使えます。
評価順序は左から右です。関数の定義順は呼び出しの可否に影響しません。
ローカルの再代入はありません。Wasm の `local.set` はこの言語のソースでは使えません。

複数束縛の `@let` は、初期値を左から右に一度ずつ評価するネストした `@let` に展開します。各初期値は、それまでの束縛と外側のスコープを参照でき、後続の束縛は参照できません。同名の束縛は、その初期値を以前のスコープで評価した後から以前の名前を隠します。body からは最後に束縛された名前が見えます。型注釈は束縛ごとに省略できます。これは複数の初期化をまとめる形式で、Wasm の複数戻り値を受ける構文ではありません。pair の分解には既存の pattern let を使います。

`@let` には `(@let $x (type i64) 42 body)` のように任意の型注釈を書けます。
注釈があれば初期値がその型に適合することを検査し、束縛には注釈した型を保持します。
例えば `(@let $x (type (ref null $T)) (struct.new $T ...) body)` の `$x` は nullable です。初期値の式自体の型は non-null のままです。

## GC データ

WAT 形式の struct・array を使用します。フィールド名は任意で、アクセスには名前または添字を使えます。

```wat
(type $Node (struct (field $value i64) (field $next (ref null $Node))))
(type $Cell (struct (field $value (mut i64))))
(type $Ints (array (mut i64)))
```

コア命令は `struct.new/get/set`、`array.new/new_fixed/get/set/len`、`ref.null/is_null/as_non_null/eq` です。
名前付き構築は `(@struct.new $T ($field initializer) ...)`。型の全フィールドをちょうど一度指定します。
`(@struct.new $T {:field initializer ...})` と `(@$T::{:field initializer ...})` はその糖衣です。
名前付き初期値の直下では、フィールドの期待型に従った裸の数値・textリテラルも使えます。2つの糖衣は独立で、一般の式位置へ省略を広げません。
初期値はソース順に一度ずつ評価し、最後に型定義順へ配置します。opaque・effect・所有権・identity・mutableの意味を維持します。
型引数は `@struct.new` の型オペランドに既存の `(@inst ...)` を使います。[名前付き構築](docs/struct-construction.md)に詳細があります。

`(@let {:field $binding ...} subject body)` は non-null の共有構造体から指定フィールドを取り出します。対象式を一度評価し、記述順に値を読み、本体だけで使える不変ローカルに束縛します。可変フィールドも束縛時の値を保持します。未知・重複フィールド、重複束縛、空のpatternは拒否します。
`(@struct.update $T object ($field $old new-value))` は、対象を一度評価し、可変フィールドの旧値を一度読んで更新式へ渡し、正常終了した値を一度書きます。unit を返し、共有可能な非unitフィールドに限定します。旧値の束縛は更新式だけで有効です。更新式の中断・例外には通常の規約を適用し、最後の書き込みに至らなければ既存の副作用だけが残ります。原子性は持ちません。どちらも既存の型・effect・所有権・opaque検査を通す糖衣です。[詳細](docs/high-level-forms.md)。
`array.new_fixed` の要素数は WAT と同様に明示します。`set` は宣言が `mut` の場合だけ認めます。
null、配列境界、オブジェクトの同一性、割当て失敗は Wasm の意味に従います。
nullable 参照の検査による暗黙の型絞り込みは行いません。必要なら `ref.as_non_null` を明示します。
ポインタと整数の相互変換や、生の線形メモリ操作はありません。

## Option/Result の値型

```wat
(func $make (result (@inst $option (@inst $result i64 i32))) (@effect pure)
  (@option.some (@inst $result i64 i32) (@result.ok i64 i32 (i64.const 42))))

(@match (@inst $option i64) (result i64) (@option.none i64)
  (@case none () (i64.const 0))
  (@case some ($value) (local.get $value)))
```

型は `(@inst $option T)` と `(@inst $result T E)`。入れ子、引数・戻り値・フィールド・配列要素・明示 capture・loop 状態に使えます。
同じ型引数の組合せは1つの内部型に具体化します。型引数が異なる値を暗黙には変換しません。
関数の結果宣言で Result 値を返す場合は `(result (@inst $result T E))` と書きます。標準 WAT の複数戻り値の宣言とは文脈で区別します。

構築は `(@option.none T)`、`(@option.some T value)`、`(@result.ok T E value)`、`(@result.err T E value)` です。
型引数には unit も使えます。その側は payload を持たず、構築時の value と case の束縛を省略します。
例えば `(@result.ok $unit i32)`、`(@option.some $unit)` です。unit 自体はフィールドや引数などに保存する型としては使えません。

match の型指定には上のような型構築子を渡し、Option は none/some、Result は ok/err の各variantを明示ケースまたは末尾のdefaultで扱います。
payload の個数・型と網羅性を検査し、値がない場合も case の空の束縛列 `()` は必要です。
none と some(null)、none と some(unit)、ok(unit) と err(unit) は異なる値です。

Option/Result は意味上の値であり、格納用の GC 参照を直接操作する型ではありません。ref.eq/is_null/as_non_null や struct.get/set で格納表現を操作できません。
payload に通常の GC 参照を持つ場合、その参照の identity や可変性は引き続き payload 自身の規約に従います。
バックエンドは型ごとの非公開の GC subtype を生成しますが、これは型付き IR 上で Type::Value として区別します。

ローカルで構築し、match/drop だけで消費する共有の tuple/Option/Result の格納用割り当ては、未使用ローカル・複数の match・分岐・小さな関数のインライン化を含めて除去できます。default を含む payload は元の順で一度だけ評価し、通常の参照 payload の同一性と明示的な trap は維持します。構築側の try/label/loop のスコープは越えません。外へ逃げる値、owned/resource/borrow を扱う関数、同一性を持つ enum、未知の呼び出し、未確認のケースは格納表現を残します。公開 ABI のシグネチャと外部値の tag/subtype 検査を保ちます。非公開の格納用割り当てとそれによる OOM の箇所は消える場合があります。この compiler TCB の最適化は、新しい構文や汎用の multi-value 呼び出し規約を追加しません。[値の最適化](docs/value-optimization.md)を参照。

## 閉じた enum と網羅的な match

enum は `@variant` 型として宣言します。ケース名は `$` を付けない ASCII 識別子で、先頭は英字または `_`、以降は英数字・`_`・`.`・`-` です。

```wat
(type $Result (@variant (ok i64) (error i32)))
(type $List (@variant (nil) (cons i64 (ref $List))))

(@variant.new $Result ok (i64.const 42))

(@match $Result (result i64) (local.get $result)
  (@case ok ($value) (local.get $value))
  (@case error ($code) (i64.const -1)))
```

型ごとのケース集合は閉じており、1ケース以上が必要です。重複するケース名は認めません。
各ケースは0個以上の数値・GC 参照の payload を持ちます。payload は不変で、構築時に宣言順に一度ずつ評価します。
再帰型・前方参照を認め、構築結果はその enum 型の non-null 参照になります。
構築時の個数・型を検査するので、`(ref $List)` が必要な位置に `ref.null` は渡せません。
参照先が明示的に可変なオブジェクトなら、その参照先の変更は可能です。

`@match` は `(@match $Type [(result R)] subject (@case name ($bindings...) body) ...)` です。
subject は指定した enum 型の non-null 参照でなければならず、一度だけ評価します。
ケースの順序は自由ですが、各variantを明示ケースまたは末尾のdefaultでちょうど1回ずつ扱う必要があります。未知のケース・重複・分岐漏れはエラーです。

末尾の `(@default body)` は、残りのvariantをpayload束縛なしで扱います。重複・途中の配置・余分なdefaultを拒否し、省略するpayloadが所有値・借用値の場合も拒否します。型・作用・所有権検査と実行時のタグ・subtype検査は維持します。Walzの分岐は `@case`・`@default` を使い、標準Component WATの `case` 宣言は元の綴りを維持します。
束縛の個数は payload と一致させ、型はケースの宣言から決まります。同一 case 内の束縛名の重複はエラーです。
束縛はその case の本体内だけで使え、外側の同名束縛を隠せます。選ばれた本体だけを評価します。
各本体の型は result と適合させます。result を省略すると unit です。末尾位置の match は各 case に末尾位置を引き継ぎます。
末尾の `@default` 以外のワイルドカード、ガード、入れ子のパターンはありません。入れ子の分解には別の match を書きます。

ソースで enum の内部を `struct.new/get/set` することや、別の enum 型として扱うことはできません。
`@variant.new` は引数のないケースも毎回割り当てます。`ref.eq` は参照同一性を比較し、構造的な等値比較は提供しません。

Wasm には不変の i32 タグを持つ基底 GC struct と、ケースごとの final な subtype を出力します。
各 subtype はタグに続いて、そのケースに必要な payload だけを保持します。タグ分岐と検査付き `ref.cast` で分解します。
この基底型は Wasm の境界では sealed ではありません。外部 Wasm が渡す未知のタグ・基底型そのもの・タグと矛盾する payload 表現は、match の防御的な検査で trap します。
外部入力を事前に再帰検査する仕組みはありません。検査は各 match の実行時に行い、副作用解析では `may_trap=true` と保守的に扱います。
enum の構築も割当て失敗を含む `allocates=true`・`may_trap=true` です。不変 payload の読み取り自体は `reads` を発生させません。

## 状態を引数で渡すループ

```text
(@loop $label
  (bind $state T initial-value) ...
  [(result R)]
  body)

(@recur $label next-value ...)
```

初期値は外側のスコープで左から右に評価します。loop の body では state が読めます。
body が通常の値を返すとループを終了します。`@recur` は最も内側のループの末尾位置でだけ使えます。
recur は各引数を左から右にすべて評価してから、すべての state を同時に更新して反復します。
個数・型は bind と一致させます。この反復は Wasm の loop/br に変換され、呼び出しスタックを増やしません。

## for マクロ

`(@for-each pattern list-value unit-body)` は共有可能な非unit要素の不変リストを順に走査します。リスト式・長さは一度だけ評価し、各要素を一度読みます。patternは単一名、tuple分解、構造体のフィールド分解。空なら本体を実行せず、本体と全体はunitです。内部ラベルや添字は公開しません。早期脱出には外側のblock/brやreturnを使えます。型付きのlet/list/loop/if/block/recurへ展開し、外側の所有値の反復消費を既存の検査で拒否します。[詳細](docs/high-level-forms.md)。

```text
(@for $label
  (bind $state T initial-value) ...
  (while i32-condition)
  (step next-state ...)
  unit-body)
```

1個以上の state を宣言します。初期値は外側のスコープで左から右に一度ずつ評価します。
各反復では条件を評価し、0なら終了します。非0なら本体、step の順に評価し、全 state を同時に更新します。
step は bind と同じ個数・型が必要です。本体が例外を投げた場合は、その反復の step を実行しません。
束縛は反復内で不変で、再代入のための構文は追加しません。for は loop/if/block/recur の型付き IR に展開します。
本体は unit、for 全体も unit です。本体中の recur による for の直接操作や break/continue の専用構文は提供しません。

## Wasm 例外の try/catch マクロ

```wat
(tag $Error (param i32 i64))

(@try (result i64)
  (do (throw $Error (i32.const 1) (i64.const 42)))
  (catch $Error ($code $value) (local.get $value))
  (catch_all (i64.const -1)))
```

tag は通常の Wasm tag です。payload は0個以上の数値・GC 参照で、throw の個数と型を静的に検査します。
throw は値を返さない never 型です。引数は左から右に評価し、エンジンの例外として送出します。
try の do と各 handler は result と適合させ、省略時は unit です。
catch の束縛は不変でその handler 内だけに有効です。個数・型を tag と対応させ、同一 handler 内の名前の重複を拒否します。
catch の tag 重複を拒否し、catch_all は最後にだけ置けます。対応しない例外は外側へ伝播し、handler 内で投げた例外も同じ try では捕捉しません。
handler の末尾位置も維持し、外側の loop への tail recur を利用できます。

展開には現在の Wasm の `try_table`・catch の分岐先 block・payload のローカル束縛を使います。戻り値コードや独自の例外ヒープに置き換えません。
**trap は例外と別です。** 整数ゼロ除算、null、配列境界、unreachable などの trap は catch_all でも捕捉しません。
これらの規則は [Wasm exception handling](https://github.com/WebAssembly/exception-handling/blob/main/proposals/exception-handling/Exceptions.md) に従います。
例外参照の取得・再送出が必要な標準 WAT では `catch_ref`/`throw_ref` をそのまま使えます。型付きマクロへの exnref 束縛や finally は未実装です。
`may_throw` は関数内で throw を実行する可能性を表し、内部で捕捉するものも含めます。may_trap と区別して呼び出し先から伝播します。
throw の割当てと資源枯渇も保守的に作用へ含めます。handler の到達可能性・例外の完全な除去は証明しません。

## 明示的なクロージャ

```text
(@closure $C
  (@capture $captured T) ...
  (param $argument U) ...
  [(result R)]
  body)

(@closure.new $C capture-value ...)
(@closure.call $C closure-value argument ...)
```

宣言はモジュール直下に置き、宣言ごとに固有の型 `(ref $C)` を作ります。
本体は capture・param・本体内の let/loop/for/match/catch の束縛だけを参照できます。捕捉の推論はありません。
捕捉値は構築時に宣言順で評価して値として保存します。GC 参照を捕捉すると参照先オブジェクトは共有されます。
環境フィールドは不変です。状態の変更には、明示的に `mut` なフィールド・配列を持つ参照を捕捉します。
呼び出しではクロージャ値を最初に、次に引数を左から右に一度ずつ評価します。

各クロージャ型の本体は1つなので、呼び出し先は静的に決まります。環境を GC struct、呼び出しを通常の直接 call に変換します。
この規約により、捕捉の寿命を手動管理せず、閉じた呼び出しグラフを構築できます。
同じシグネチャを持つ別のクロージャ宣言への暗黙の型変換はありません。

## テスト宣言

```wat
(@test "name" i32-expression)
```

宣言はモジュール直下に置き、名前は空白のみではない文字列です。同一モジュール内の同名テストを拒否します。
本体は引数・暗黙の捕捉を持たない関数として検査し、結果型は `i32` に固定します。
本体内では通常の let・loop・match・明示的な closure 構築と、モジュールの関数・型を使えます。
他の関数と同じ規則で `never` 型の本体も受け入れますが、実行時の trap とタイムアウトはテスト失敗になります。
正常に0を返す場合は失敗、非0を返す場合は成功です。異なる数値型からの暗黙の変換はありません。

型付き IR の `Module.tests` は、名前・ソース範囲・本体の FunctionId・実行用 export 名を持ちます。
本体は通常の関数と同じ IR・副作用解析・コード生成を使います。生成した関数名はソースから参照できません。
実行用 export は利用者の export と衝突しない名前にし、通常の check/types/ir/analyze/build もテスト本体を処理します。
Wasm 内にテスト登録用の可変配列や動的な呼び出し機構を生成しません。

ファイル探索・名前による絞り込み・結果表示・制限時間はホスト側の `runner` の責務です。
各テストは新しいインスタンスで実行し、テスト間の GC オブジェクト共有や実行順への依存を言語機能として提供しません。
絞り込みはモジュール全体の型検査後に行います。実行するテストが0件、またはモジュールに型エラーがある状態を成功とは扱いません。

## 最適化に使う意味論上の前提

実行時のオブジェクト管理は Wasm GC に委ね、線形メモリ上の独自ヒープや生のポインタ演算を導入しません。
ローカル、クロージャ環境、enum のタグと payload は不変です。可変な struct/array の変更はソースと IR に明示します。
ただし GC 参照の別名は許すため、異なる引数が別のオブジェクトを指すという仮定は置きません。
クロージャ呼び出し先と全ての通常の call は静的に決まり、テストの追加もこの性質を変えません。

最適化後も、左から右の評価順序、実行される分岐、明示した変更、参照同一性、定義された trap を保つ必要があります。
不変フィールドの読み取りは可変メモリの read 作用を持ちませんが、nullable な参照の読み取りの trap は別に扱います。
GC の実行時期やオブジェクトのアドレスを取得する機能、finalizer、弱参照は提供しません。
import は全副作用を保守的に持ち、利用者の任意の注釈で pure とみなすことはできません。
これらの事実を型付き IR と作用の要約に保持します。型付き IR 上で、条件を満たす非公開の値の格納表現を除去し、検証済みの不変な範囲を最適化します。範囲検査の削除には、後述の診断用 may 解析とは別の must 解析を使い、Wasm GC 配列自体の境界検査は残します。[値の最適化](docs/value-optimization.md)・[範囲の最適化](docs/range-optimization.md)を参照。

## 外部境界と解析

import/export は WAT の関数境界です。外部関数の実装は埋め込み側が与えます。
import には副作用を過小評価する注釈を認めず、読み書き・割当て・trap・非停止・ホスト作用を保守的に仮定します。
型付きコアの全命令を検査して IR に変換し、そこから解析と WAT 生成を分岐します。標準 WAT 経路は Wasm validator を使い、コアの解析ができない部分に純粋性を仮定しません。
解析結果の false は、その作用がこのコアのモデル内で生じないことを表します。true は可能性であり、必ず生じることを意味しません。
実行時のスタック枯渇、ホストの資源制限、デバッガによる介入などは、この副作用モデルの対象外です。

## effect 契約

```wat
(func $add (param $x i64) (param $y i64) (result i64)
  (@effect pure)
  (i64.add (local.get $x) (local.get $y)))

(func $raise (@effect (throws $Error))
  (throw $Error (i64.const 42)))
```

関数と closure のヘッダに `(@effect ...)` を指定できます。省略時は実装から推論します。
項目は reads、writes、allocates、suspends、host、synchronizes、blocks、`(throws $Tag...)`、`(throws *)` です。pure は空の集合の別名で、他の項目と併記できません。
指定した集合は許可する作用の上限です。実装の作用が上限を超えた場合は、契約の位置と不足する作用を報告します。
重複、未知の項目・tag、複数の effects 宣言を拒否します。契約は本体より前に置きます。

reads/writes は可変 GC データや線形メモリの読み書き、allocates はソースで identity を観測できる値の生成です。
通常の struct/array、既存の参照型 variant、closure の構築は allocates を持ちます。
Option/Result/pair/own とローカル配送端点の格納用割当て自身は semantic allocates を持ちませんが、payload を評価した際の reads/writes/allocates などは必ず合成します。

throws は関数外へ出る可能性のある tag 集合です。対応する catch が body 内の tag を取り除き、catch_all は body の未知の例外も処理します。
handler 自身の throw は同じ try では取り除きません。Result の err は throws を追加しません。
呼び出し元は呼び出し先の実装から推論した作用を受け継ぎ、再帰を含めて固定点まで伝播します。宣言された契約で実際の作用を隠すことはできません。

未知の import は全作用と throws(*) を持ちます。利用者が import に effect 契約を付けることは拒否し、将来の信頼された binding 契約と分離します。
effect 宣言の reads/writes/host は大域的な上限です。別途、analyze の memory は読み書きの対象と escape、facts は制御フロー上の事実を報告します。対象別の契約構文や callback の effect 型引数は未実装です。

`analyze` は semantic_effects と declared_effects を表示します。従来の effects は物理的な GC 割当て・内部の throw・trap・非停止などを保守的に表します。
pure は停止や trap が起こらないという証明ではありません。物理的な割当て失敗なども従来の effects に残ります。
raw WAT/外部コードに GC 表現を露出した値の最適化では、semantic な契約だけで identity・割当てを消さず、lowering の作用と escape も検証する必要があります。

### 詳細な memory / facts 解析

memory は、引数・捕捉の番号、割当て元の関数・位置・内部 slot、unknown を root とする may-access path を使います。
member はフィールド、配列要素、variant/value の payload です。reads/writes は既存の semantic effect と同じく、可変 GC スロットへのアクセスを表します。
reads/writes、参照を格納する heap、返す参照、ホストへ渡す参照、例外経路へ渡す参照を、呼び出しグラフと loop/recur に沿って固定点まで伝播します。
引数と捕捉を実引数の由来へ代入し、更新は古い値を残す weak update とします。呼び出し先の引数へ格納したことだけで、呼び出し元の非公開な割当てが外へ出たとは扱いません。
各割当てについて、返却、外部から到達可能なスロットへの格納、ホスト呼び出し、例外経路からの到達可能性を求めます。循環と container/capture の参照も辿ります。例外経路の thrown は、呼び出し元まで漏れず捕捉される場合も保守的に含めます。
引数・捕捉同士は別名になり得ます。allocation は箇所単位の抽象で、各呼び出し・反復に固有のオブジェクトを表すものではありません。深さ3を超える参照パスと未知のコードは unknown へ広げます。
未知の書き込みや中断の後では、可変スロットにある参照を unknown も含む集合へ広げます。不変フィールドや論理的な値の payload 自体は変更可能と仮定しません。

facts は、関数内 preorder の node ID と元の span を持ちます。マクロの生成ノードでは span が重なるので、span だけを一意な ID として扱いません。
i32_range は 0..2^32-1 の区間で、演算は折り返し・シフト量のマスクを含む Wasm の意味に従います。区間で安全に表せない結果は全範囲とします。i64・浮動小数点の区間推論は未実装です。
変更されないローカルの値・別名と、GC 配列の不変な長さの対応を追います。`i < len` の unsigned 比較、または non-negative が確認できる signed 比較を、境界の根拠として保持します。
ArrayNew の length 引数が別の配列長やローカルの値を写した場合も、そのスナップショットの等しさを保持します。これは配列オブジェクト同士の同一性を意味しません。
null 検査と正常に通過した dereference を用いて non-null を精密化します。可変フィールドを繰り返し読む操作を、同じ値だとは仮定しません。
枝の合流では区間を包含し、条件は共通部分を残します。loop の持ち回り値には初期値を流用せず、宣言型の上限へ widening します。不変条件や停止性の証明は行いません。
normal_return_possible=false は正常な継続がないと確認した場合です。true は必ず成功するという保証ではありません。bounds_proven/null_check_proven の false は未確認を表します。

これらは診断用の may 解析と局所的な事実です。既存の effect 宣言・所有権の合否を変えず、出力する Wasm の削除判断には直接使いません。非 escape、pure、診断の境界フラグだけでは、identity・評価順序・trap の削除を認めません。
`proof/Flow.lean` は signed/non-negative の境界条件を BitVec で検査します。Rust の抽象解釈器や出力全体の健全性を証明するものではなく、負の添字の反例、区間と折り返し、別名・ループ・呼び出しは回帰テストでも確認します。

独立した作業量制限付きの must-range パスは、不変な値のスナップショット、unsigned のガード、検査付きアクセスの正常終了を追います。小さな共有値の関数を、ローカル・ループの識別子を刷新してインライン化できます。各反復の持ち回り値は未知から始め、可変フィールドの再読込や未対応の呼び出し・制御構造は根拠にしません。証明した部分範囲は offset の折り返しなく合成できます。冗長な論理分岐を除いても元の条件の評価は残し、証明済みの固定幅 reader は helper の明示的な検査だけを省き、Wasm GC の境界検査を保ちます。ソースから unchecked read を要求する方法はありません。未証明のケースは検査を残し、最適化前にソース全体を検証します。`proof/Ranges.lean` は算術規則の証明であり、Rust 実装や出力 Wasm の正しさ全体を証明しません。

## WIT interface の生成

`walz wit file.walz` と `wit::emit(&module)` は型付きコアの公開関数から WIT を生成します。
固定の package `walz:generated@0.1.0`、interface api、world program を使う型の契約です。この `wit` コマンド自体は実行形式を生成しません。実行用の Component は別の `component` コマンドで生成します。
内部関数とテスト関数は公開せず、引数名は p0/p1/...、export 名は元の lowercase kebab-case を保ちます。WIT のキーワードは escape し、無効な export 名は拒否します。

i32/i64 は既定の境界規約として s32/s64、f32/f64 は同名の WIT 型へ変換します。符号なし整数の意味型はまだありません。
`v128` は WIT/JavaScript の値境界に直接対応せず、入れ子も含めて拒否します。スカラーや bytes などへの明示的な変換が必要です。Wasm 内部の値には使えます。
Option/Result は再帰的に同じ WIT 型へ変換します。Result<Unit,E> は result<_,E>、Result<T,Unit> は result<T>、Result<Unit,Unit> は result です。pair は tuple に変換し、own は明示的な resource adapter を要求します。
Option<Unit> のように WIT が単位値の payload 型を必要とする場合は、1ケースの補助 enum を使います。
Future/Stream は future<T>/stream<T> に対応し、unit の payload は引数のない future/stream にします。
中断可能性が推論された関数には async を付けます。WIT に表せない GC 参照や、関数外へ出る未処理例外がある export は明示的な adapter を要求します。
WIT は任意の effect 契約を保持しないため、生成 interface だけで相手の関数の pure を仮定することはできません。

## 所有値の最小規約

```wat
(@let $ticket (@box.new i64 (i64.const 42))
  (@match (@inst $box i64) (result i64) (@move $ticket)
    (@case own ($value) (local.get $value))))

(@let $unused (@box.new $unit)
  (@box.close (@move $unused)))
```

`(@inst $box T)` は共有可能なデータ T を包む、複製できない値です。T は unit でもよく、その場合 own.new の値引数と case の束縛を省略します。
own.new は新しい所有値を作り、move はローカル・引数・捕捉値の所有権を渡します。所有値に local.get は使えません。move 後の再使用、所有しない値の move、所有値の drop を拒否します。
own は case own の match で消費して内容を取り出すか、own.close で明示的に消費します。暗黙の解放はありません。
現在の own.close は所有するデータの包みを消費し、Wasm の drop に展開します。OS リソースの close や Future/Stream の cancel を実装する操作ではありません。
own は payload の GC 参照を排他的にしません。既存の共有 GC 参照の別名や変更可能性はそのままです。own の内部へ別の所有値を直接包むことは認めません。

`(@inst $tuple T U)` は `(@tuple.new T U first second)` で作り、case pair の match で分解します。unit 成分の値引数と束縛は省略します。
pair、Option、Result は、成分のどれかが所有値なら全体も所有値です。所有値を捕捉する closure も所有値になり、構築時の所有 capture と呼び出し時の環境に move を使います。
所有 closure は nullable にできず、ref.eq/is_null/as_non_null で参照を観測できません。自身を呼び出すと消費され、捕捉した所有値も本体で処理する必要があります。
共有される struct/array/参照型 variant のフィールド、例外 payload への所有値の格納は拒否します。

正常に戻るスコープでは、引数・捕捉・let・case・loop の所有値を、移動・返却・分解・close のいずれかで処理する必要があります。
初版は affine の自由な放棄を認めず、この範囲では線形の終了規約を使います。共有値は従来どおり何度でも使えます。
if/match の正常に合流する枝は、残っている所有ローカルの集合を一致させます。recur は同時更新の引数を評価した後、外側の所有集合をループ入口と一致させ、古い所有 state を残してはいけません。
状態を move して次の state に渡すことや、古い state を消費して新しい state を作ることはできます。
停止しないループの終了や、trap・OOM の際のリソース解放は証明しません。明示的な unreachable へ進む場合には、その時点の所有値を処理済みにします。

所有値と try/throw・例外を投げる呼出しを併用できます。catch・分岐・反復の入口の所有集合と、評価途中の所有 operand も検査します。所有値を失う例外経路は拒否します。
一般の所有値を受け渡す未知の import、所有する例外 payload、部分 move、一般的な lifetime は扱いません。外部 resource には同期 destructor と非流出の borrow を提供します。[制御と所有権](docs/core-expressions.md) を参照してください。

## 配送済みの Future と有限 Stream

```wat
(@let $future (@future.ready i64 (i64.const 42))
  (@future.await i64 (@move $future)))
```

Future/Stream は readable end に対応する opaque な所有値です。直接の match・ref 操作、共有 GC フィールドへの格納、暗黙の drop は認めません。
通常の core Wasm 出力では、Wasm GC に格納した配送済みデータを扱います。Component 出力では pending と JSPI のイベントループにも接続します。共有 payload の writer とスコープ付き spawn / join も扱います。[writer と子タスク](docs/scoped-tasks.md) を参照してください。
payload T は共有できるデータに限ります。所有値や別の Future/Stream を含む payload は、終了時に黙って放棄しないよう初版では拒否します。
Result のような共有データの組合せ、GC 参照、unit は使えます。payload が GC 参照でも、その参照先の共有・変更可能性は維持します。

| 操作 | 型・意味 |
|---|---|
| `(@future.ready T value)` | Future<T> を作る。unit なら value を省略 |
| `(@future.await T future)` | 端点を消費して T を返す。suspends を持つ |
| `(@future.close T future)` | 受け取りを放棄し、端点を消費。unit を返す |
| `(@stream.empty T)` | 終端の Stream<T> を作る |
| `(@stream.cons T value rest)` | value を先頭とする Stream<T> を作り、rest を消費。unit なら value を省略 |
| `(@stream.next T stream)` | 端点を消費し、pair<Stream<T>,Option<T>> を返す。suspends を持つ |
| `(@stream.close T stream)` | 残りの受け取りを放棄し、端点を消費。unit を返す |

構築時の引数は左から右に一度ずつ評価し、読み取りの対象式も一度だけ評価します。
await で返した Future は再利用できません。next の後は返された Stream だけを使います。
要素があれば次の端点と some(value)、終端なら終端の端点と none を返します。終端を再読しても none です。終端でも所有値が返るため、最後に close が必要です。
none と some(unit)/some(null) は区別します。Stream の終端だけでは、別の completion Future の成功や失敗は決まりません。

await/next には、中断境界として suspends を常に付けます。通常の core Wasm では配送済みデータを即時に返し、Component 出力では外部の配送も待ちます。
構築と close 自体は host/suspends を追加しません。物理的な GC 割当て・検査付き cast の trap は従来の低水準の解析に残します。
ローカル端点の close は消費だけです。Component の外部端点なら drop-readable へ変換し、ホストの dispose/iterator.return を促します。OS I/O、進行中の読み書き、producer task 全体の取消しは保証しません。

`examples/ready-delivery.walz` は、pair<stream<i64>,future<result<unit,i32>>> を作り、40と2を読み、終端を close し、completion を確認して42を返します。
同じ所有権の契約を run-verified と Component 生成で検査できます。Component 境界では次節の Canonical ABI adapter を使います。
所有端点を raw Wasm export に渡すことは引き続き契約付き実行で拒否します。

`proof/Delivery.lean` は、一度限りの Future 配送、closed 端点の拒否、終端でも Stream の所有者が残ること、next を繰り返した結果が take/drop に一致することをモデル上で証明します。
このモデルも生成する Lean ソースに含めます。実際の Wasm の意味保存は証明の対象外として、配送順序・終端・unit・GC 参照・評価回数を実行テストで照合します。

## bench

```wat
(@bench "integer mix" (param $i i32) (result i32)
  (^ (* $i 16777619) (>>> $i 7)))
```

`@bench` はモジュール直下の実行用宣言。非空かつモジュール内で一意の名前文字列、
任意の `(param $i i32)`、任意の `(result type)`、式1個をこの順に置く。
省略時は引数なし・unit。本体は通常の関数と同じ型・effect・所有権の検査を受ける。
結果は共有可能な値またはunitで、所有値・borrowを返せない。名前からソース内の関数として参照できない。
IRの `benches` に名前、Span、FunctionId、export名を記録する。新しい式の意味論は追加しない。

`walz bench` は既定でカレント以下の `*_bench.walz` を再帰探索する。
実行は同期的な閉じたguestコードに限る。ホストimportと中断を実行時ハーネスで受け付けない。
各ケースを別プロセスで実行し、Wasm内のバッチループから本体を反復番号0〜N-1で呼ぶ。
結果は公開globalに保存し、最後のGC結果だけを保持する。計測用のloop/globalはランナー専用で、
通常のcompileでは追加しない。測定条件とCLIは[ベンチマーク](docs/benchmarking.md)を参照。

## law と契約付き実行

```wat
(func $add (param $x i32) (param $y i32) (result i32) (@effect pure)
  (i32.add (local.get $x) (local.get $y)))

(@law "addition is commutative" (param $x i32) (param $y i32)
  (i32.eq (call $add (local.get $x) (local.get $y))
          (call $add (local.get $y) (local.get $x))))
```

law は名前、0個以上の param、i32 の条件式を持ちます。名前の重複を拒否し、param の全入力で条件が非0になることを要求します。
条件は既存の式で書き、別の述語言語を増やしません。law 本体の effect 上限は pure です。law はソースから生成した専用関数になり、export は持ちません。
実際の law 関数と呼び出し先の型付き IR を Lean の BitVec に変換し、bv_decide の証明を Lean のカーネルで検査します。i32/i64 の全ビットパターンを対象とし、整数の折り返し、符号付き比較、Wasm のシフト量のマスクを保ちます。
初版は i32/i64 の定数・対応する非 trap 演算、local/let/block/if、非再帰の直接 call、unit のみです。除算・剰余、再帰・ループ、浮動小数点、GC・所有値・例外などは law の意味モデルの対象外として拒否します。
証明が失敗するか、検証器が使えない場合は実行・出力を拒否します。Lean の maxHeartbeats は4,000,000で、予算内に証明できない性質も未証明として扱います。

`verify` と `run-verified` は型付きコアのみを受け付け、全 import、所有値を渡す raw Wasm export、effect 上限のない公開関数を拒否します。生成した test・bench 関数の effect 宣言は不要です。
所有ローカルを扱う契約を JSON に抽出し、Lean の独立した検査器でも検査した後、その同じ IR から Wasm を生成・validate します。利用者が契約を弱めて実装の作用や所有操作を隠すことはできません。
law を含むモジュールでは、通常の run/build/wat/expand と CLI check もこの検査を通します。law のない通常の WAT 経路は従来のままです。
Rust API check と CLI types/ir/analyze は検査・調査用で、law の証明を実行しません。実行形式を作る compile、契約付きの verified::compile では省略できません。

`contracts` で所有権の義務、`proof` で生成した Lean ソースを確認できます。検証器は Lean 4.34.1 に固定します。
`proof/Ownership.lean` は、検査に成功した契約が宣言的な所有集合の規則を満たすこと (check_sound) と、二重消費を拒否すること (double_take_rejected) を証明します。
これは契約モデルの証明です。構文解析、Rust の型検査・effect 解析・契約抽出・BitVec への変換、Wasm への lowering、runtime/adapter は信頼する実装として残ります。コンパイラ全体の意味保存、停止性、I/O 成功を証明したものではありません。
参考にした [Bend の law/proof と quantity](https://github.com/bendlang/bend/blob/3ca3df3209ebb0fac4b80e41dac18b41442e6c58/guide/GUIDE.md) と同様に、契約、実装、検証器を分離します。Bend の依存型理論そのものは導入しません。

## 設計規約: effect と WASI 0.3.0 の対応

この節は実装の到達点と次の拡張の設計規約です。値型、effect 契約、所有権、ready-only の Future/Stream と WIT 型の生成、law と Lean による検査、解析結果の `may_suspend`、[WIT の形状契約](interfaces/async-contract.wit) は実装済みです。
外部 readable handle の pending 読み取り、Component adapter、JSPI 上の timer binding とタスクの協調的な取消しを実装しています。取消し時は進行中の read を終了させます。共有 payload の writer と言語側の producer / consumer task も実装しています。

参照先は [提示された Verse の effect モデル](https://verselang.github.io/book/13_effects/) と [WASI 0.3.0](https://github.com/WebAssembly/WASI/releases/tag/v0.3.0) です。
effect を関数の型に含め、許可された作用の上限として検査する方式を採用します。ロールバック付き失敗、prediction、Verse 固有の effect の組合せ制限は初版の規約に含めません。
外部 I/O を伴う計算が err を返しても、既に完了した I/O を巻き戻す意味は与えません。

### データ型と制御を分ける

| 言語側の意味 | WIT への対応 |
|---|---|
| `Option<T>`: none / some(T) | `option<T>` |
| `Result<T,E>`: ok(T) / err(E) | `result<T,E>` |
| `Result<Unit,E>` | `result<_,E>` |
| `Future<T>`: 一回の値配送を受ける所有ハンドル | `future<T>` |
| `Stream<T>`: 順序付きの値列を受ける所有ハンドル | `stream<T>` |
| ハンドルを返してから別の配送を待つ関数 | `func(...) -> future<T>` |
| 呼び出しの完了そのものを待つ関数 | `async func(...) -> T` を基本の binding 方針にする |

Option/Result は意味上の値型で、GC での格納レイアウトから分離します。none と some(null) を同一視しません。
格納用 GC オブジェクト自身の identity は値型の観測対象にせず、payload の意味上の identity はその型の規約に従います。
成功・失敗側の payload が省略される WIT result と、unit を配送する引数なしの future/stream も境界で正規化します。
T/E は境界で WIT に表現できる型である必要があり、WIT の型検査を再利用します。
内部の循環 GC グラフ、クロージャ、GC 参照を直接 WIT の値として公開することはしません。必要なら値表現への変換または resource handle を明示します。

Result の err と Option の none は通常の値です。これらを返すだけでは throws/decides の effect は発生しません。
`future<Result<T,E>>` は通常の組合せとして許可し、suspends と Result を組み合わせることも許可します。
Result 伝播の `@result.let` / `@result.do` は match に展開し、その式の Result を返します。Wasm throw や関数からの暗黙 return へ変換しません。
Wasm 例外、trap、task cancellation、読み書き操作の中断、配送完了、stream の終了、アプリケーションの err は別に扱います。
内部例外から公開 Result への変換が必要な場合は、対応する tag と error variant を adapter に明示します。

### 関数の effect と推論

関数型は概念的に `Fn(Params) -> Results ! Effects` とし、実装から推論した作用が宣言した上限に収まることを検査します。
内部関数は推論を基本とし、公開 API と binding には契約を残します。Wasm validator は命令の型を、effect 検査は追加の作用契約を検査します。
callback の型にも effect を保持し、汎用関数は callback の作用を自身の作用へ合成します。

| family | 追跡する事実 |
|---|---|
| Heap | 対象領域ごとの reads/writes、観測可能な新しい identity の生成 |
| Protocol state | channel/endpoint ごとの reads/writes。配送・close・cancel の状態変化 |
| Suspension | 論理的に中断・再開し、他の処理が介在できること |
| Exceptional control | 関数外へ出る throws の tag 集合 |
| Divergence | 非停止の可能性 |
| Host | filesystem、network、clock など、利用する capability |

現行の `analysis.allocates` は物理的な GC 割当ての可能性です。意味上の値のための割当てと、参照同一性で区別できる値を生成する作用を混同しません。
意味上の関数契約と、GC 割当て・trap などの lowering 後の解析情報を別に保持します。
現行の `may_throw` は内部で捕捉する throw も含む保守的な情報です。公開 throws 契約と semantic_effects は、捕捉後も関数外へ出る tag を使います。

`may_suspend` は GC pause や OS のスケジューリングを意味しません。await/next、未知の import/opaque WAT に保守的に true を設定し、呼び出し元へ伝播します。
WIT は型と async の呼び出し形式を表しますが、任意の関数の reads/writes/purity の証明は含みません。
import の作用を狭める場合は、実装と対応する信頼された binding 契約を使います。利用者が arbitrary な import を pure と宣言して検査を回避することは認めません。

中断点は共有可変状態に対する最適化の境界です。await の前後の読み取りを共通化するには、参照先の不変性や他タスクから変更されないことの証明が必要です。
future を返す計算や task を開始した計算については、producer の作用を task/scope の要約に保持します。呼び出し元から suspends が消えても、開始した I/O や変更まで消えたとは扱いません。
Future は配送端点であり、producer の関数型や task そのものと同一視しません。

### 所有権と終了処理

GC オブジェクトは従来どおり参照を共有できます。ローカルの Future/Stream は移動後の再利用を禁じる所有値です。将来の外部端点と resource の own handle にもこの規約を適用します。
所有値を含む Option/Result/pair/closure への所有性の伝播は実装済みです。将来の resource 型・record にもこの規約を適用します。利用者が自由に共有できる GC フィールドに所有値を格納する操作は認めません。
compiler が生成する async frame の所有スロットは、状態遷移と解放を検査する内部表現として別に管理します。
WIT の future/stream 値に対応するのは readable end です。writer は別の内部型とし、暗黙に ABI の future/stream 値として渡しません。
複製や複数回 await が必要な場合は、共有・バッファリングのための別の明示的な抽象を使います。

read/write が進行中の端点と ABI バッファは貸出状態とし、完了通知または中断の確認前に再利用・移動・解放しません。
所有ハンドルを捕捉する closure にも移動を適用し、本体がその値を消費する場合は once-call として検査します。
捕捉変数の宣言を維持し、所有 capture の move も明示する記法を設けます。
borrowed resource は呼び出しの期間にだけ有効とし、非同期呼び出し中もその寿命を維持します。

端点の close、進行中の read/write の cancel、task の cancel は別操作です。ハンドルを drop しただけで producer task を強制終了したとは扱いません。
task cancellation は要求と完了の確認を分け、通常の戻り値との競合も runtime protocol に従って処理します。
終了処理は GC finalizer の時期に依存させず、正常終了・明示移動・例外・cancel の経路に所有権の解放を定義します。
関数から stream/future を返した後も producer が動く場合を表現し、戻り値の配送と producer task の終了を区別します。

### WASI 0.3.0 の I/O 形状と ABI

ストリーミング I/O は `tuple<stream<T>, future<result<_,E>>>` を基本形に揃えます。
stream の終了だけで操作全体の成功を確定せず、completion future の Result も確認します。エラーを各要素の Result に自動で押し込めません。
backpressure と、chunk 単位の読み書き・部分的な進捗を adapter の protocol に残します。

内部の値・継続状態は Wasm GC に置き、component 境界では [Canonical ABI](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md) に従って lifting/lowering します。
端点の所有権と中断・終了処理の根拠は [Component Model の concurrency 設計](https://github.com/WebAssembly/component-model/blob/main/design/mvp/Concurrency.md#streams-and-futures) を参照します。
境界用の linear-memory buffer と handle table を使う層を分離し、内部 GC の参照をそのまま ABI に渡す前提は置きません。
WASI package は `@0.3.0` を固定し、Component Model/生成ツール/runtime の組合せも記録します。main branch の将来の gated feature を自動で 0.3.0 の契約に追加しません。

`interfaces/async-contract.wit` はこの対応を検証する独自のテスト interface で、wasi:* interface の実装ではありません。
`just wit-check` は WIT の encode/validate/decode の往復を検査します。これは型の形状の検査であり、WASI runtime 上の非同期実行や GC adapter の完成を意味しません。
共有 payload の writer、スコープ付き spawn / join、WASI 0.3.0 のファイル値 adapter と利用者定義の単相化を実装しています。子だけの取消し・期限と、importしたresourceのnative own/borrowに対応しています。native WASI一式のresource world、select、所有 payload の配送は後続です。

### 実装した Component と JSPI の契約

`component source.walz output.wasm` は型付きコアだけを受け付けます。公開関数の effect 上限、固定した import の名前・シグネチャ、所有権、law を検査し、Lean を通した同じ IR から Component を生成します。
公開 API は `walz:generated/api@0.1.0`。数値、Option/Result、pair、Future/Stream を再帰的に変換します。GC 参照と own box の公開、未知の import、未処理例外は拒否します。内部では GC の循環グラフやクロージャを使えます。

内部の共有値は GC の値表現、外部の Future/Stream は readable handle を持つ非公開の GC subtype です。await/next は対象式を一度だけ評価し、端点を消費します。
外部 await は `canon future.read`、next は `canon stream.read` へ変換します。同期 Canonical ABI の read が終わるまで JSPI のスタックを中断します。Wasm の GC 参照はこのスタック上で到達可能なまま保持します。
Stream の read 結果の個数と状態を別々に解釈し、DROPPED と同時に最後の1要素が届いた場合も保存します。終端で外部 handle を drop し、再読可能なローカルの終端へ置き換えます。

境界の線形メモリは16 MiBを上限に8バイト境界で管理します。数値・variant の discriminant と payload・tuple の配置は Canonical ABI に従い、16 flat parameter を超える引数は間接渡しです。
間接引数は GC 値へ変換してから元の領域を解放します。複合戻り値は呼び出し固有の領域を返し、post-return で解放します。進行中の read 用領域は読み取り完了と lifting の後に解放します。
trap 後はインスタンス全体を破棄する前提です。破棄後のバッファ再利用や、trap 時の外部操作すべての自動取消しは保証しません。

配送済み Future と有限 Stream の前置要素は、生成した `walz:runtime/delivery@0.1.0` の factory に渡します。factory は Promise または pull 型の AsyncIterable を返します。
producer をゲスト内で走らせないため、戻り値の消費を待ってゲスト呼び出し自体が戻れなくなる循環を作りません。Stream の外部 tail を前置要素に接続でき、close はまだ読んでいない tail にも伝播します。
unit は future/stream では payload なし、Result では省略 payload、Option/pair の要素では1ケースの `walz-unit` enum に変換します。none と some(unit)、unit 要素と EOF を区別します。
`.wasm.json` は factory と async export の対応を記録する version=2 の manifest で、Component と対にして配布します。

信頼する event-loop import は `walz:host/event-loop@0.1.0` の sleep/yield/defer-i64/ticks/fallible-i64 だけです。名前だけでなく型も一致する必要があります。全て host を持ち、sleep/yield は suspends も持ちます。
Future を返すだけの defer は suspends を持ちませんが、タイマーの開始という host 作用は保持します。任意の import の effect を利用者が狭める仕組みは追加していません。
端点の読み取り・close の所有契約は生成先でも維持します。現在の大域的な effect 集合には端点ごとの protocol state は含まれず、pure だけを根拠に所有操作を消去する最適化規則はありません。
`analysis` の低水準の割当て・trap 情報は既存の core lowering についての要約で、生成した ABI glue とホストの全操作の解析ではありません。

JS 実行は Node 24.21.0 / Chromium 156 の Wasm GC と JSPI、jco-transpile 0.23.0 を検証環境とします。ネイティブの Component async 関数を同期 Canonical ABI で lift/lower し、同一インスタンスの呼び出しを直列化します。JavaScript のイベントループは待機中も進行します。
`runtime/jco-compat.mjs` はこの固定バージョンの生成 JS について、unit のバッファ情報、undefined の Stream 要素、失敗した async import の確定処理を補正します。補正後の glue も信頼する実装の一部です。
Lean の Delivery モデルは ready-only のままです。pending 状態、ABI の意味保存、jco のスケジューラや Promise の完了を証明したものではありません。これらの接続は Node/Playwright で実行して照合します。

### Core＋JSPI の JS 専用 ABI

`core-js source.walz output-directory` は、上の Component 出力と同じ型・effect・所有権・law の検査を行い、同じ IR から `core.wasm`、`memory.wasm`、`bindings.mjs`、`manifest.json` を生成します。
`runtime/build-core.mjs` は、これに共通の `core.mjs` / `host.mjs` / `tasks.mjs` と `index.js` を加えます。JS のビルド・実行に jco をロードしません。
公開できる型、信頼する import、未処理例外と配送 payload の制約は Component 出力と共通です。GC 参照や own box の公開、所有 payload・入れ子 Future の配送は引き続き拒否します。

manifest の `abi` は `walz-core-js-v2`。Core の loader はこの値を検査し、v1 の数値 binding も受け付けます。生成 Wasm・bindings・共通ホストは同じ ABI の組として配布します。
これは JS 向けの private ABI であり、標準 Component の Canonical async ABI / WASI future・stream handle の実装ではありません。
数値・variant・pair・bytes/text のメモリレイアウトとアリーナは既存の adapter を共有します。引数は常に間接渡しで、引数があれば packed tuple のポインタ1つを渡し、全て GC 値へ lifting した後に元の領域を解放します。
unit 以外の戻り値もポインタを返し、生成した JS が値をコピー・端点を移譲した後、finally 内の post-return で領域を解放します。unit 戻り値は値を渡しません。

JS の数値は i64 が bigint、それ以外が number。s32/s64 の範囲は入力時に検査します。bytes は Uint8Array、text は string としてコピーします。
pair は2要素の配列、Result は `{tag:'ok'|'err', val}`。unit は Option/pair 内で `'value'`、Result・Future/Stream の省略 payload では undefined です。
Option は none を undefined、some を payload で表し、直接の payload が Option なら外側を `{tag:'none'}` / `{tag:'some',val}` にします。
公開 API とモジュール内の portable Future/Stream の payload に必要な型ごとの read/write 関数を生成します。非公開の GC 型や内部だけの text のために JS codec を生成しません。

外部 Future/Stream だけを、インスタンスごとの JS 表で正の u32 ID に対応付けます。GC オブジェクトの root 表は持ちません。
await/next は JSPI の import を中断し、値が届いてから一時領域に書きます。Future の consume/close、Stream の EOF/close、取消し時に登録を外します。
境界から返す端点は表から取り出して呼び出し元へ所有権を移し、その端点の JS wrapper は Wasm インスタンスを保持しません。ID は再利用せず、u32 の上限で失敗します。
Future を返す同期タスクの完了と、その Future 自体の配送完了は区別します。返却した未受信の Future は明示的に close するか、close するゲスト関数へ戻せます。

同じインスタンスへの呼び出しは既存の Tasks で直列化します。`instantiate()` の `api` / `start`、`getCoreModule`、`instantiateCore`、`eventLoop`、`imports`、`onTaskEvent` を共有します。
ゲストに入る前の JS 入力変換に失敗した場合は、その変換で登録した端点とバッファだけを解放します。Wasm trap・host の失敗はインスタンスを閉じ、以降の呼び出しを拒否します。
host の失敗はソースの catch_all が捕捉しても fatal とし、外側の呼び出しを失敗させます。元の理由は Error 本体または RuntimeError.cause に保持し、0 や undefined の reject も失敗として扱います。
取消し時は未完了の read を切り離し、Wasm の checkpoint と専用例外による unwind を経てバッファを解放します。遅い producer の完了が解放済み領域へ書く callback は残しません。
任意の Promise 自体の停止は強制せず、host の dispose / iterator.return が完了してから取消しの終了を通知します。詳細な状態遷移は次節と共通です。
この接続の意味保存は Lean の証明対象に追加していません。両出力に同じ Node/Chromium テストを適用し、端点の登録数・アリーナ・GC 到達性を実測します。

### JSPI タスクの協調的取消し

`instantiate()` が返す `start(jsExportName, args, {signal?})` は、同じインスタンスの待ち行列にタスクを追加します。task は readonly の state と result、`cancel(reason?)` を持ちます。
state は queued/running/cancelling/returned/cancelled/failed です。cancel は終了確認の Promise を返します。終了確認の値は `{status: returned|cancelled|failed, started: boolean}` で、再要求は同じ Promise を返し、最初の理由を保持します。
result はゲストの通常の結果、TaskCancelledError、実行または解放の失敗のいずれかで確定します。取消し理由は TaskCancelledError.cause です。公開 API の Result や throws tag の集合へ暗黙には変換しません。

queued の取消しではゲストに入らず、引数を渡しません。running の取消しでは要求フラグを立て、進行中の native Future/Stream read にキャンセル完了イベントを発生させます。待機中の sleep/yield にはホストの dispose を要求します。
Canonical read の完了またはキャンセルの通知を受けるまでは、ゲストのバッファを解放しません。読み取り後とホスト呼び出し後の checkpoint で要求を確認し、未初期化の payload を lifting する前に内部の専用 tag を throw します。
ソースの try/catch の lowering は、この tag を user handler より先に検出して外へ再送出します。catch_all も取消しを吸収できません。外側の ABI wrapper がこれを捕捉し、所有端点とバッファを解放してから JS 側へ戻します。

外部 readable handle を作る時に、非公開の GC 所有リストへ登録します。消費・close・境界への移動時にはリストから外し、取消し時には残る端点だけを drop-readable します。所有 closure や組の中に移した端点も、このリストで追跡できます。
アロケータは live block の双方向リストを持ち、通常の free でリストから外します。取消しの unwind が始まった後にだけ、残った ABI 用領域を free-all します。
スコープ内で開始した非同期ホスト解放をすべて待ってから、cancelled を通知します。解放が失敗した場合は failed と AggregateError を返し、以後の呼び出しを拒否します。成功した取消しでは同じインスタンスを再利用できます。

通常の返却が先に確定している場合は returned が勝ちます。返却済みの Future/Stream は呼び出し元の所有物であり、後から task.cancel しても drop しません。Future の配送待ちを、Future を返す非 async な関数の完了と混同しません。
同期の直接 API は同期値を返し、他タスクの実行・解放中の呼び出しは busy で拒否します。start を使えば同期関数もキューへ追加できます。async API の直接呼び出しは従来どおりキューで直列化します。

取消しは CPU 処理を強制停止せず、既に行った副作用を巻き戻しません。ホストの操作や dispose が終了しない場合には、終了確認も完了しないことがあります。
現行の [Component Model の取消し規約](https://github.com/WebAssembly/component-model/blob/main/design/mvp/Concurrency.md#cancellation) は、callback ABI のタスクへ要求と確認を配送します。この実装は同期 ABI/JSPI 用の walz の制御層であり、native `subtask.cancel` / `task.cancel` を直接使う実装ではありません。writer と言語側の producer task は既存の所有権と JSPI に接続します。ゲストから子だけを取り消す構文は追加していません。
jco の補正には pending-copy のキャンセル接続、取消し完了後にホストの iterator.next を待ち続けない処理、ホスト解放の終了確認を含めます。これらも runtime の TCB です。

`proof/Tasks.lean` は、要求が解放を完了させるわけではないこと、read 終了前のバッファ解放拒否、全資源・ホスト解放の終了前の acknowledgment 拒否、返却後の取消しが無効であることを状態モデル上で検証します。
モデルと実装の対応、Wasm の unwind、jco の copy/table 実装、外部ホストの停止性を証明するものではありません。Node/Chromium で、開始前・開始後・read 中・返却後、遅い配送、解放の遅延・失敗、再利用とメモリ再利用を照合します。


## bytes / text と ABI

`$bytes` は不変な任意のバイト列、`$string` は不変な検証済み UTF-8 の値型です。
両者とも non-null で共有でき、同一性・格納参照・書込みは公開しません。
内部では packed i8 の private GC 配列を wrapper に格納し、可変な配列への書込みを構築時だけに制限します。
数値から bytes への格納は Wasm i8 と同じく下位8ビットを使います。

| 式 | 結果と規約 |
|---|---|
| `(@bytes.new i32-expr...)` | `$bytes`。空も可。各式は左から一度だけ評価 |
| `(@bytes.from-array $Array array-expr [start length])` | 非 null の i32/i8/i16 配列の全体または範囲を不変 bytes へコピー。選択要素が0..255以外なら trap。packed 値は符号なしで読む |
| `(@bytes.copy-to-array $Array destination at bytes start length)` | 不変 bytes を可変 packed i8 配列へ一括コピー。unit を返し、範囲外は trap |
| `(@bytes.len bytes-expr)` | `i32` のバイト数 |
| `(@bytes.get bytes-expr index)` | `i32` の 0..255。index は符号なし、範囲外は trap |
| `(@bytes.slice bytes-expr start length)` | `$bytes`。start <= size かつ length <= size-start。範囲外は trap |
| `(@string.lit "日本語")` | `$string`。ソースリテラルは UTF-8 |
| `(@string.byte-len text-expr)` | `i32` の UTF-8 バイト数 |
| `(@string.bytes text-expr)` | `$bytes`。内容を保つ。private storage は共有可能 |
| `(@string.from-bytes bytes-expr [start length])` | `(@inst $option $string)`。UTF-8 が不正なら none、正しければ some。範囲指定時は検証してからコピー |
| `(@bytes.is-utf8 bytes-expr [start length])` | i32 の 0/1。payload を割り当てず、全体または指定範囲を厳密に検証 |

不正な continuation、切れた系列、overlong、surrogate、U+10FFFF より大きい値を拒否します。
NUL・BOM は通常のデータとして保存します。正規化、grapheme、コードポイント添字はこの API の対象外で、検索・比較・連結は core ライブラリが提供します。
text の byte-len は JS string.length と異なります。bytes の任意の部分列は UTF-8 とは限らないため、text への変換には常に検証を使います。

省略可能な start/length は符号なし i32 として start <= size、length <= size-start を順に検査する。オペランドは左から一度ずつ評価してから範囲と UTF-8 を検査する。範囲外は trap、範囲内の不正 UTF-8 は 0/none。空範囲は周辺のバイトにかかわらず有効。検証と ASCII の先読みは選択範囲だけを読む。範囲付き `@string.from-bytes` は検証後にその範囲だけ独立した不変 storage にコピーし、1引数形式の共有動作は維持する。opaque view では `$bytes-view.is-utf8` / `$bytes-view.to-string` を使う。`$string-view.from-bytes-view` は検証した範囲を共有する option<string-view> を返し、小さな GC header だけを割り当てる。view は元のバッファを保持し、その後の slice は文字境界を要求する。実体化すると元バッファから分離し、現在は再検証する。型・effect・所有権・opaque 検査は維持し、validator と lowering は引き続き compiler TCB。[view](docs/views.md#validate-a-byte-range-as-a-string-view) を参照。

WIT は bytes を `list<u8>`、text を `string` とします。内部 GC 参照は ABI に出しません。
[Canonical ABI](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md) の UTF-8 設定を使い、どちらも ptr/byte-length の2つの i32へ flatten し、線形メモリ上の size=8 / align=4 として処理します。
入力は GC にコピーして入力バッファを解放し、出力は専用のバッファへコピーして post-return で解放します。
空の入力は非割当てのポインタでも受け付け、解放しません。Option/Result/pair の payload も再帰的に処理します。
Future/Stream の payload は read の realloc で受け取り、配送 factory へ渡した一時領域も複合 payload ごと解放します。
アダプタの線形メモリ上限は全体で16 MiBです。領域不足・不正な ABI 入力は trap とし、text.from-bytes の none と区別します。
JS では bytes は Uint8Array、text は string です。文字列の lowering で単独 surrogate が来た場合の UTF-8 化は JS binding の TextEncoder に従います。

これらの操作自体は意味上の reads/writes/allocates を持ちません。引数の作用は合成します。
物理的なコピー・検証・割当てと、その失敗の可能性は低水準の分析に残します。
生の core Wasm に自分でアクセスするホストは、この private 表現を守る TCB の外側にいるため、値型の不変性を破らないことが必要です。

## 静的モジュール

1ファイルが1モジュールで、宣言は既定で非公開です。`@namespace` はありません。

`math.walz`:

```wat
(module
  (@export (func $add) (type $Box))
  (type $Box (struct (field $n i32)))
  (func $add (param $n i32) (result (ref $Box))
    (struct.new $Box (i32.add (local.get $n) (i32.const 2)))))
```

`main.walz`:

```wat
(module
  (@use "./math.walz" (func $add $make) (type $Box))
  (func $main (export "main") (result i32)
    (struct.get $Box $n (call $make (i32.const 40)))))
```

`(@export (func $f) (type $T) (tag $Error))` は静的リンク向けの公開です。`@public` は同義です。closure の func と type は別々に公開できます。
`(@use "./file.walz" (func/type/tag $member [$alias])...)` は選択した公開宣言だけを束縛します。別名を省くと元の識別子を使います。
組み込みの型・関数は `(@use "walz:core" ...)` から選びます。ファイルパスは `./` または `../` から始めます。絶対パス・名前空間名の指定・循環は拒否します。
同一 canonical path の定義と型の同一性を共有し、別ファイルの非公開名や依存側の import は暗黙に取り込みません。
識別子のドットは区切りではなく文字です。`$math.add` を使いたければ、その識別子を宣言するか `@use` で別名として束縛します。
import した別名を `@export` で再公開できます。衝突・未定義名・非公開名へのアクセスは拒否します。wildcard、default export、動的ロード、初期化時コードはありません。
`@export (type $T)` は表現を公開し、`@export (opaque $T)` は操作を宣言ファイル内に制限します。

`@include "relative.walz"` は宣言のテキスト挿入で、スコープを作りません。パスは記述したファイルから解決します。
ファイル解決は `source::load` / CLI が担当し、文字列の check/compile API はファイルI/Oを行いません。未解決のファイル `@use` は `E_SOURCE_REQUIRED` です。
深さ64・展開後16MiBの上限を適用し、診断を元ファイルの位置へ対応付けます。

型・effect・所有権・opaque を全定義で検査した後、ABI export 等から到達しない定義を落として単一Wasmへ静的リンクします。実行時のモジュールオブジェクトはありません。
ネイティブWATの `(import "host" "member" ...)` と `(export "name" ...)` の意味は維持します。`@use` / `@export` とは別です。
詳細は [静的モジュールとDCE](docs/static-modules.md) を参照してください。

`@use "./library.wasm"`は、ホストimportのないWIT付きComponentから同期・共有値のexportを選択する。world直下の関数は`$name`、interface内は`$interface.name`で指定し、バイナリからWITの型を得る。呼出しは`host` effectを持つ。asyncとFuture／Stream／resource境界は選択位置で拒否し、未選択のexportには影響しない。Component出力へ一度だけ埋め込み、未使用の依存全体はDCEする。内部コードの個別DCEやCore出力への合成は行わない。[型・ABI・取消しの制約](docs/wasm-use.md)。

## 値だけを渡す text I/O capability

`walz:host/text-io@0.1.0` は固定した信頼境界です。`read-lines` は `stream<result<text,i32>>`、`write-line(text)` は `future<result<unit,i32>>` を返します。
通常の I/O 失敗は Result、ホストの契約違反や cleanup の失敗は実行の失敗として区別します。両関数の作用は host であり、端点を返す呼び出し自体は中断せず、await/next が suspends を持ちます。
名前と型は `bindings::validate` で検査し、公開関数の effect 上限と所有権・law は既存の検査を通します。言語の構文や所有権規則は追加しません。

Component と Core の manifest は import ごとの module/name/jsName/params/result/capability/effects を記録します。
必須 capability は read-lines が text-input、write-line が text-output です。JS の `options.imports` は版を含む module 名をキーとし、宣言された関数を明示的に渡します。
提供された JS 実装と I/O 操作は TCB であり、manifest 自体が JS の効果を証明するものではありません。`eventLoop` の既定 binding は filesystem/network の権限を追加しません。
Component は namespace ごとに別の import instance を作り、Core は生成した bindings が型付きの引数をコピーして host へ渡します。
Core の生成物は ABI v2 として再生成します。v1 の出力にも新しい loader を使えますが、古い loader は v2 を拒否します。

### 行の表現と終了

`createTextIO` の入力は byte iterable、出力は `write(Uint8Array, AbortSignal) -> Promise<void>` です。片方だけを提供でき、未提供の操作を必要とするゲストはインスタンスの生成時に拒否します。
入力 capability は一回だけ消費します。読み取りは pull 型で、1回の next は1行を配送します。LF を取り除き、LF の直前の CR も取り除きます。単独の CR はデータとして残します。
UTF-8 は行が揃った時点で厳密に検証し、末尾の不完全な符号列も拒否します。BOM と NUL はデータとして保存し、Unicode の正規化は行いません。
末尾が LF なら余分な空行は作らず、末尾に残った改行なしの行は一度配送します。空の入力はそのまま EOF です。
蓄積用の byte buffer は幾何的に拡張し、行の上限は既定1 MiB、設定範囲は1 byte〜8 MiBです。上限は LF を除いた入力 bytes で数え、CR は含めます。

UTF-8 不正は1、read の失敗は2、write の失敗は3、行長超過は4、消費済み入力または中止された write は5を返します。
read の失敗では source を閉じて Result.err を一度配送し、その後は EOF です。write は text を UTF-8 にして LF を付け、sink の完了を Future の完了にします。
アプリが返す Result.err はタスクとしては通常の returned であり、trap と混同しません。

入力の return は進行中の next を切り離し、source.return の完了を待ちます。write の Symbol.asyncDispose は AbortSignal を通知し、sink の完了を待ちます。
ソース内の checkpoint/unwind と所有端点の drop、バッファ解放、非同期 cleanup が完了してからタスクの取消しを確認します。入力・sink が終了処理に協力することが必要です。
閉じた reader は source・読み込み中の chunk・蓄積 buffer を外し、source.return の不要な戻り値も保持しません。

### Node CLI

`text-cli.mjs output-directory [--input file] [--output file]` は main を引数なしで呼び、`result<i32,i32>` を受け取ります。既定の接続先は stdin/stdout、ファイルは host 側で遅延して開きます。
`examples/io/text-filter.walz` は空行を除いて出力し、成功時は行数を返します。stdout に行数や manifest を混ぜません。
終了コードは成功0、Result.err または実行失敗1、SIGINT による TaskCancelledError は130です。SIGINT 後も cleanup の終了を待ち、読み書き途中のファイルを閉じます。
同じ path または同じファイルを指す入力・出力は、書き込み前に拒否します。出力ファイルは明示的に選んだものを作成・上書きし、空の出力でも空ファイルにします。
Node の handle 管理は runtime/node-io.mjs に分離します。通常完了で借用した stdout を閉じることはせず、factory で開いたファイルは end/close まで待ちます。Node 版の取消しでは入出力の stream を閉じるため、一回の実行スコープへ接続します。

この WIT と接続コードは walz 固有であり、wasi:* package の実装や汎用の WIT import generator ではありません。既存の Lean モデルを拡張した意味保存の証明は追加しておらず、Rust/Node/Chromium の実行検査で接続を照合します。

## 文字列ライブラリと配列のコピー

`@bytes.from-array` の引数は指定した配列型の非 null 参照で、各オペランドを左から一度ずつ評価する。結果は独立した不変スナップショットで、その後の配列への書込みで変化しない。start/length は符号なしで start <= size、length <= size-start を満たす必要があり、選択した要素だけを変換する。`@bytes.copy-to-array` は不変 bytes を読み、指定配列に書き込む writes 効果を持つ。bytes の private 格納領域を公開しない。

型付き GC 配列は標準 WAT の packed i8/i16 を受け付ける。値の型は i32 で、格納時は切り詰め、読取り時は `array.get_s` / `array.get_u` で符号拡張方法を明示する。unpacked 配列には従来の `array.get` を使う。`array.copy $Destination $Source dst at src start length` は左から評価し、Wasm の範囲検査・null・重複範囲のコピーの意味を維持する。型付き部分では可変コピー先と数値要素／格納型の一致を要求し、可変コピー元の reads とコピー先の writes を追跡する。参照配列のコピーは標準 WAT validator 経路に残る。両方の型に opaque 検査を適用し、list/array-view 変換は unpacked 格納を要求する。

固定幅読み取りは `(@bytes.read-FORMAT-ENDIAN bytes offset)`。FORMAT は i16/u16/i32/i64/f32/f64、ENDIAN は le/be。`$bytes.read-*` と `$bytes-view.read-*` を import して呼ぶこともでき、view ではその可視範囲を検査する。i16/u16/i32 は i32、それ以外は対応する Wasm 数値型を返す。i16 は符号拡張、u16 はゼロ拡張。`(@array.write-FORMAT-ENDIAN $Array destination offset value)` は i16/i32/i64/f32/f64 に対応し、指定した可変 packed i8 配列の非 null 参照を要求して unit を返す。i16 は下位16bitを格納する。浮動小数点はビット再解釈で、Wasm 内では符号付きゼロや NaN payload を維持する。JS number 境界での NaN payload 保持は保証しない。オペランドを左から一度ずつ評価後、offset <= size、width <= size-offset を符号なしで検査する。非整列を許可し、不完全な範囲は trap。書き込みは全範囲を検査してから変更する。読み取りは pure、書き込みは writes で、opaque・所有権検査を省略しない。現在の出力は GC アクセスとシフトで、線形 scratch memory は不要。[固定幅アクセス](docs/byte-access.md) を参照。

`walz:core` は `$i32.checked-add-u`、`$i32.checked-mul-u`、`$i32.checked-align-up-u` と i64 版を提供する。同じ幅のスカラー2つを受け取り、pure 効果で同じスカラーの Option を返す。入力は最上位ビットを含めて符号なしビット列として扱う。加算・乗算は数学的な結果が符号なしの幅を超えれば none。整列は非ゼロの2の累乗に切り上げ、不正な整列値・overflow は none。最上位ビットの2の累乗も有効。成功したゼロは some(0)。通常の call として引数を左から一度ずつ評価する。Wasm の wrapping、数値型、ABI の符号付き格納は変更せず、アプリケーションの上限は呼び出し側で別に検査する。既存の型・effect・所有権検査を通るライブラリ関数であり、構文や未検証の算術仮定を追加しない。[検査付き整数](docs/checked-integers.md) を参照。
元配列と格納域を共有せず、元配列を後から変更しても bytes/text の内容は変わらない。
可変配列を読む場合は reads を持ち、memory 解析にも対象の要素読み取りを残す。返す値から元配列への参照はない。
新しい bytes 自身に観測可能な identity はないので、格納用の物理的な割当てと semantic allocates は区別する。

ルートの宣言位置で `(@include "walz/core/text.walz")` を使う。include のパスは呼出し元からの相対位置に合わせる。
`$bytes.concat(bytes,bytes)`、`$string.concat(text,text)`、`$string.u64(i64)`、`$string.s64(i64)` を提供する。JSON文字列の構築・出力は `$json.stringify` と `$json.from-string` を組み合わせる。
u64 は64 bitのビットパターンを符号なし十進数へ、s64 は符号付き十進数へ変換する。
json は引用符を含む JSON string を返し、quote/backslash/制御文字をエスケープする。
これらは通常の walz の関数で、ローカルな GC 配列の構築・変更を含むため reads/writes/allocates の上限を持つ。
可変な作業配列と、公開する不変の bytes/text はコピーで分離する。concat の計算量は入力長の合計に比例する。

検索・分割・上限付き整数解析と text/JSON builder の API は [walz/core/README.md](walz/core/README.md) に定義する。
検索位置は符号なしの UTF-8 byte 単位であり、text の検索開始位置が文字の途中なら none を返す。
整数解析は乗算前の上限検査を行い、空・不正範囲・桁あふれを option.none として扱う。
builder は通常の不透明な可変 GC 型で、finish が断片への参照を外して再利用可能な空の状態に戻す。
JSON object builder はフィールド名・値のエスケープと区切りを管理し、入れ子の child は finish して取り込む。
これらは言語の組込み型や構文を増やさず、既存の型・effect・opaque 検査に従う。Lean による意味保存の証明は追加していない。

## Result と Stream の限定マクロ

```wat
(@result.let ($value) (@inst $result T E) U subject body)
(@stream.fold-result T S E source initial ($state $item) body)
```

result.let は subject を一度評価し、ok の payload を body に束縛する。body の結果は result<U,E>。
err は同じ E のまま伝播する。初版の E は共有できる値に限定する。T=unit なら束縛を `()` とする。E=unit も扱う。
成功 payload が所有値の場合、body での明示 move と消費を既存の検査器が要求する。

stream.fold-result は stream<result<T,E>> を消費し、共有できる非 unit の状態 S を持つ。
source、initial の順にそれぞれ一度だけ評価し、body を各 ok payload に対して評価する。body は result<S,E> を返す。
EOF は Stream を閉じて ok(state)、入力の err と body の err は残りの Stream を閉じて err を返す。
状態 S は非 unit とする。T=unit の場合の束縛は `($state)` だけとする。
body は同じ関数内の loop/match に展開され、動的な callback や closure を生成しない。

一時名は全ソースの識別子と衝突しないよう生成し、通常の名前解決・型・effect・所有権検査を展開後に行う。
展開後の深さ上限は256、ノード数上限は100万。中断・取消しの protocol は既存の task scope を使う。
任意の throw を処理する finally ではない。明示的な abrupt exit で端点が未消費なら従来どおり拒否する。
result.let の err 経路に別の所有値が残る場合も拒否する。

## opaque な公開型

```wat
;; counter.walz
(module
  (@export (opaque $State) (func $new) (func $get))
  (type $State (struct (field $n (mut i32))))
  (func $new (result (ref $State)) (struct.new $State (i32.const 0)))
  (func $get (param $s (ref $State)) (result i32)
    (struct.get $State $n (local.get $s))))
```

外部では nominal な型として参照・受渡しできる。通常の `(@use "./counter.walz" (type $State $Alias))` も使える。
構築、フィールド／要素の読書き、variant の match、配列長取得と bytes.from-array は、宣言したファイルの中だけで認める。
別名経由と、型が推論された参照経由にも同じ検査を行う。ref.eq、nullability と GC の意味は既存の規則を使う。
検査は型付き IR の操作と元の宣言スコープを照合し、追加の実行時ラッパーや ABI は作らない。
フィールドごとの個別公開と friend は初版に含まない。

## ソースの整形と CLI の診断

`walz fmt file.walz` はそのファイルだけを整形し、`fmt-check` は未整形なら終了コード1を返す。include の参照先へ自動的に書き込まない。
文字列のエスケープ、行コメント、入れ子のブロックコメントを保持し、トークン間の空白だけを変更する。
括弧・文字列・コメントが閉じていなければ書き込まない。閉じていない括弧は開始位置を報告する。
formatter は型検査の代わりではなく、標準 WAT のスタック構文にも使える。
text CLI の Result err は `program error <code>` と表示し、アプリケーションのエラーも含む。
EOF で完了した正常な main、Result の err、SIGINT の終了コードは従来どおり0、1、130。

## 透明な型 alias と局所的な型引数の省略

`(@type.alias $Name T)` は型の出現箇所に展開する透明な略記です。型名の名前空間を使い、GC の type / closure と同名にはできません。
前方参照を許可し、循環・深さ256超・展開ノード100万超を拒否します。未使用の target も既存の型・所有性規則で検査します。
alias 自体を実行用 IR の nominal type にせず、未使用 alias の型を実行用 Wasm に追加しません。
ファイル内の `@export (type $Name)` と `@use` に対応します。alias に opaque は指定できず、target の opaque 制限は展開後も適用します。
型の位置では `$Name` と書きます。heap type ID を要求する `(ref $Name)` や `struct.get $Name` 等には使えません。
unit の alias は payload の位置に限定し、通常の関数引数やフィールドへ unit を導入しません。

次の省略形を既存の明示形に追加します。

```text
(@result.ok [value])                 ;; expected result<T,E>
(@result.err [error])                ;; expected result<T,E>
(@tuple.new [first] [second])         ;; expected pair<T,U>、unit成分は省略
(@option.none) / (@option.some value)  ;; expected option<T>
(@box.new [value])                   ;; expected own<T>
(@match subject (@case variant ($names...) body)...)
(@let ($first $second) subject body)
(@result.let ($value) subject body)
(@stream.fold-result source initial ($state $item) body)
```

値を構築する省略形は、関数の戻り値、呼出しの引数、明示した束縛型、フィールド、明示した分岐の結果型等から期待型が決まる場合に検査します。
期待型を欠く bare constructor は注釈を要求します。match 省略形は subject から variant/value 型を決め、出力には期待型を要求します。
既存の関数・if・block・長い match の結果宣言省略=unit は維持します。関数間の制約を解く全域推論や、型が決まらない箇所の推測は行いません。

pattern let は subject の pair を一度評価し、既存の単一 case の match と同じ型付き IR にします。
共有値の複製や、所有値の move・消費義務も通常の match と同じです。unit 成分の束縛は省略し、同名の重複束縛を拒否します。

短い result.let は subject から T/E、期待型または body から局所的に U を決めます。
E は共有できる同一の型です。nullable / non-null の一方への変更も認めません。所有する成功 payload は明示的に move して渡せます。
式に閉じた err 伝播であり、関数からの暗黙 return にはなりません。
短い fold は source の stream<result<T,E>> と initial から型を決め、既存の loop/match に展開して検査します。
S は共有できる非 unit、E は共有値、body は result<S,E> です。source、initial の評価順と1回ずつの評価、EOF / err の close、task の取消し確認は既存形と同じです。
コンパイル時の型探索は実行を増やさず、仮の local/loop を本体へ残しません。式の検査量には1関数100万ステップの上限を設けます。

## 外部 resource と借用

同期 destructor を持つ nominal resource、非流出の borrow、using を追加した。
詳細と所有権・例外・ホスト境界の制約は [外部資源の契約](docs/resources.md) を参照。
WIT binding の生成と対応型は [WIT binding](docs/wit-bindings.md) を参照。

## 不変の list

`(@inst $list T)` と5つの操作、WIT list<T> の変換は [list の契約](docs/lists.md) を参照。
所有権を持つ要素や借用の格納は認めない。

## 利用者定義のジェネリクス

`(@type.params $T ...)` と `(@inst $name T...)` により、関数・型・alias・クロージャを単相化する。

型および関数・heap 型の参照位置では `$name<T, U>` を同じ意味の糖衣として使える。入れ子の値型、GC 参照、呼び出し、構築、汎用クロージャ、具体化した関数の宣言的 export に対応する。`<` は名前に隣接させ、カンマで引数を区切り、内部の空白・コメント・改行と末尾カンマを許す。空や不正な引数列は `E_GENERIC_SYNTAX`。引数には既存の S 式の型も書け、参照 wrapper は明示する。その名前空間で宣言/import した完全一致の名前を優先し、`@inst` で適用を明示できる。ローカル・束縛名・引用した WAT 識別子・メタデータ・演算子は再解釈しない。元の span を保って展開し、既存の型・制約・effect・所有権・opaque 検査と具体化のキャッシュを使う。formatter と Watz 表示にも対応し、エディターの未完の構文はコンパイルしない。
適用時の検査、再帰と展開の上限、公開 ABI、map/fold は [単相化の契約](docs/generics.md) を参照。
`@type.params` 直後の where は shared/nonunit/callable、組み込みのAdd/Sub/Mul/Div/Eq/Ord/Logical/Bitwise/Shiftとcallableのeffect上限を具体化時に検査する。
固定の基本型インスタンスで演算子を解決し、制約を消去して既存の閉じたIRへ変換する。利用者定義class/implや抽象的なbody検査は追加しない。[型クラスと演算子](docs/typeclasses.md)を参照。
Logicalはi32の0/非0を真偽値とし、`&&`・`||`を短絡するifと0/1への正規化へ展開する。実行されない右辺にも型・effect・所有権の検査を課す。BitwiseとShiftはi32/i64に固定し、シフト量も同じ型にする。`>>`は符号拡張、`>>>`はゼロ埋め、シフト量のマスクはWasmの規則に従う。


## スカラー newtype と構造化された早期脱出

`(type $Id (@newtype i64))`、`@newtype.wrap` / `@newtype.unwrap`、平坦な `@result.do` を提供します。
標準 WAT の `return`、名前付き block、`br`、`br_if` は型付きコアでも検査し、抜ける `@using` の終了処理を実行します。
型・評価順・所有権証明書・ABI の消去と対応範囲は [数値と制御の規約](docs/core-expressions.md) に定義します。

## Wasm threads と共有メモリ

名前付き memory32 の定義／import と、threads の67 atomic命令、memory.size/grow を型付きIRで扱います。`shared` は最大ページ数が必須です。WAT の命令名とメモリ即値を維持し、整数アドレス・整数 payload・自然アライメントを検査します。
`@effect synchronizes` は同期、`blocks` は同期的な待機の上限です。Future の `suspends` と区別します。
所有権検査は既存の GC / 端点の move と消費に適用し、共有領域内の排他所有やホストの競合は証明しません。
[構文・境界・保証範囲・実行例](docs/threads.md) を参照してください。

## 固定 atomic セルの条件付き証明

`(@race-free)` はエントリーモジュールの全定義を検証モードにします。
`(@atomic.cell $counter $memory i32 0)` で固定セルを宣言し、`(@atomic.at $counter)` を atomic のアドレス位置に使います。セルの型はi32/i64、メモリは固定サイズのsharedを最大1つ。rawアドレス・狭幅・offset・size/grow・ホスト関数importを拒否します。
ホストが同じセル契約を守るという前提で、Leanが任意の長さのアクセス列のデータ競合排除・同じ範囲の幅の一致・範囲内アクセスを証明します。コンパイラの抽出／出力とエンジンはTCBです。
[構文・ホスト契約・機械検査と保証しない範囲](docs/atomic-cells.md) を参照してください。

## 公開後不変の共有領域とWorkerホスト

`(@readonly.region $index $memory byte-offset positive-byte-length)` と `(@readonly.load $index u8/i32/i64/v128 relative-byte-offset)` は、静的に宣言した領域内の動的な読取りを表します。アドレス式を一度だけ評価し、ロード幅を含む符号なしの範囲検査を行います。`reads`を持ち、非atomicなので非整列アクセスもWasmの規則に従います。

`@race-free`と併用するとversion 2の証明書を使い、公開後の不変性をホストの前提として、atomicセルと分離したreadonlyアクセスのtraceをLeanで検査します。このモードでは追加の定義済みunshared memoryを私有として許します。Core＋JSPIのABIはv4、共有ホスト契約は`walz-readonly-regions-v1`です。`@race-free`なしの宣言は書込みを禁止する所有権やfreezeの証明を追加しません。[規約とTCB](docs/readonly-regions.md)。

Node・ブラウザの`WorkerHost`は言語外の明示的なホストAPIです。driverの初期化、ジョブキュー、個別取消し、join、終了を担当し、データ形式やWasmの関数名はdriverへ分離します。GC参照やFuture端点をWorkerへ暗黙に移しません。`@task.spawn`の協調的実行は維持します。[配送・寿命・取消しの契約](docs/worker-host.md)。

## CLI と拡張の実行範囲

Rust 1.99 を基準とし、`walz run` / `walz build --target core-js` でホストとランタイムを明示します。[CLI](docs/cli.md) を参照してください。
`@abi.type` の record / enum / flags / unsigned と WIT の生成範囲は [WIT binding](docs/wit-bindings.md)、型付きマクロに混在できる数値メモリ命令は [validator との橋渡し](docs/wat-validator.md) に定義します。
`(@inst $task T)` は `walz:core` の所有付き子タスク型で、`@task.spawn T $Closure environment` / `@task.join T handle` / `@task.drop T handle` / `@task.cancel T handle` / `@task.join-timeout T milliseconds handle` と `@task.scope (result T) body` を使います。GC 環境・共有結果の同一性と、スコープ退出時の完了待ちを維持します。[契約と制限](docs/scoped-tasks.md)。

## 明示的な Stream batch と deadline

`@stream.read-batch T max stream` は1–65536の上限で読み、所有する tail と不変 list を返す。
空 list は EOF、非空 list は最大 max 要素の prefix。既存の一要素 next の動作は変えない。
[ABI・部分進捗](docs/stream-batches.md) と [公開呼出しスコープの期限](docs/scoped-tasks.md) を参照。

## エディターと native Component

stdio の `walz lsp` は型・effect・所有権の診断、hover、定義への移動を提供する。UTF-16、未保存の相対モジュール、上限付き cache と取消しの契約は [LSP](docs/lsp.md) に定義する。
WIT の world を指定する Component 出力は、共有 payload の async endpoint と imported resource の typed own/borrow を Canonical ABI に接続する。対応する形と実行条件は [native worlds](docs/native-worlds.md) に定義する。

## 期待型付き文字列と共有値combinator

期待型がsemanticなstring型と確定する式位置では、UTF-8文字列トークンを `@string.lit` と同じtyped Data式へ展開する。期待型がない場合や他の型を要求する場合は `E_LITERAL_TYPE`。暗黙変換、所有権、effectの規則は変更しない。
`walz:core` の `$option.ok-or` / `$result.map-err` は通常のshared/nonunit制約を持つ単相化関数。[引数の評価順とcallbackの契約](docs/option-result.md) に従う。

## 任意の表示形式

Watzのmatch分岐は `case`・`default` を使い、基本形式のWalzでは `@case`・`@default` を維持します。

ソースと `walz print` の基本形式はWalzのS式です。任意の `--format watz` 表示では `func`・`@export("name")`・直接書ける `match`／`if` 式を使い、同じ構文木を保ちます。保存した `.watz` 表示も読み戻せます。`module { ... }` は省略可能で、束縛には不変の `let` を使います。通常のモジュール解決・検査の前にS式へ変換し、型や実行時の意味論を追加しません。正規の言語形式はWATのS式のままです。構文・エスケープ・制限・コメント・ツール対応は[対応表](docs/review-syntax.md)に記載します。

## Component構成ソース

ルートの `(component ...)` はComponentを生成し、`(module ...)` はCoreの意味を保つ。
`(@lib 宣言...)` は実行ABI・start・global/data初期化・Core exportを持たない静的ライブラリ。
通常の `@use` / `@export` で選択し、単独でcheckできるがbuildのエントリーにはしない。

Component内の `(@expose "public-name" $function<具体型...>)` は、実装を単相化・検査して
portableなCanonical ABI公開関数を推論する。`(@bind "module" "member" $component-function)`
は選択したComponent関数でWalzのimportを供給する。引数名とasync性を実際の関数から取り、
最終的なComponent validatorで型の互換性を検証する。host effect/capabilityは保つ。
標準Component定義と明示的なinstantiate順序は維持する。
`(@use "./unit.wasm" (component $Constructor))` は定義を読み込み、状態は生成しない。
Componentの `.walz` ソースも同様に選択できる。インスタンスの同一性はファイルパスでなく
明示的な `instantiate` で決まる。

ソースコンストラクターは名前の直後に `(@type.params $T...)` と、任意の `@where` 制約を取る。
`(instantiate $Constructor<i32>)` と `(instantiate (@inst $Constructor i32))` は
明示的に単相化する。型引数は呼び出し側の静的な型スコープで閉じた値型や可視なnominal型定義に解決し、
標準型のimportは引き続き明示する。aliasを正規化してから特殊化をキャッシュする。
同じ特殊化は定義を共有するが、instantiateごとの状態と定義resourceの同一性は独立する。
型だけの `@lib` / `@use` は公開関数を要求せず、実行用コードを生成しない。
型の置換では値の束縛名と内側の型引数を区別し、具体化した本体にも通常の型・制約・effect・
所有権・証明・Wasm検査を適用する。バイナリは具体化済みなので型引数を取らない。
genericなスコープではComponent参照とComponentのouter aliasを名前で指定し、生成定義による
数値インデックスの参照先の変化を防ぐ。通常のnative WATとCore命令のインデックスの意味は保つ。
nominal型のコピーでは再帰的な配置・mutability・packed型・nullability・ABI注釈を保つ。
異なるソース宣言やphantom型引数は、同じ配置でも別の特殊化として扱う。
opaqueな内部表現、closure、resource/borrowの同一性はコピーできない。
公開する名前付き値型にはnativeな型portを設け、instanceのexport ascriptionで値型の名前を
付け直してComponent subtypingを検査する。resourceの同一性や実行時変換は変更しない。
内部コンストラクター単体ではWITで表せない場合があるが、最終的な `--wit` のentryにはWIT表現を要求する。

`(@world "namespace:package@version" "world")` は検証済みバイナリから取り出すWITを命名する。
`build --wit` は出力の隣に `world.wit` を生成する。`--wit-input FILE [--world NAME]` は
既存の契約を読み、Moduleではnative worldを生成し、Componentでは公開ポートと型を検証する。
WITで表せないCore Module exportも標準Componentとして有効で、WITを要求した場合に限りエラーにする。

`(@component.type "binding" (param "function" index-or-name))`、`(result "function")`、
`(type "member")` で、明示的な接続から値型を取り出す。carrier・field・case・tupleの型を辿り、
WITのABI注釈と実際のresourceの同一性を保つ。実装内の照会は通常の検査前に解決する。
コンストラクターの型引数では、その適用より前にある検証済みの接続先を照会できる。
先行する特殊化済みinstanceも対象にでき、bind宣言自体は後置できる。
各段階では引数から到達する照会だけを解決し、実際の型で制約を検査する。
型推論中にinitializerやhost関数は実行しない。前方・循環依存は `E_COMPONENT_TYPE_PHASE` にし、
探索用の仮の型は実装IRに渡さない。
照会だけでhost import・destructor・権限を追加しない。名前付き値は公開型のportを保持し、
resourceには明示的なソース宣言を要求する。未知のselectorや不適切な文脈はエラーにし、
opaqueな注釈はデータとして保つ。[接続した型の照会](docs/component-composition.md#connected-type-queries)を参照。

生成アダプターのload/storeはABIメモリを名前で参照し、ソースのメモリ参照は保つ。
canonical操作は特定のメモリを選ぶ。メモリの同一性はCoreインスタンス単位で、import aliasは
同一性を共有し、新規instantiateの定義メモリは別物になる。解析できないホスト・aliasは
未解決として報告し、最適化やrace-freeの証明にはしない。endpointのバッファは完了・ackまで
保持し、キャンセル要求だけでは解放しない。nativeのtrap/cancellationはinstance/storeの寿命に
従い、任意のnativeホストにSDKのcleanup保証は適用しない。

値の推論は共有データ・Future/Streamに対応する。さらに `(@bind "namespace" (instance $target))`
からimported resourceの契約を推論し、`@expose` に適用する。ソース側のown/borrow・move・
destructor importは明示したままcanonical dropに接続する。import型aliasはvalidatorの同一性で
解決し、表現や名前が似た別resourceを同一視しない。呼び出せるのは宣言したimportだけで、
同じnamespaceにinstance形式と関数形式のbindを重ねない。Option/Result/pairの所有値と、所有関数の
単相化も同じ検査を通す。raw GC参照・v128・externref・未確定の型引数・所有するlist/record/endpoint
payloadは未対応。コンストラクターの型引数では型定義をコピーし、GC agent間でheapオブジェクトは
コピーしない。opaqueな配置とresource/borrowの同一性は型引数の対象外とする。v128・box・task型は内部実装の特殊化に使えるが、公開シグネチャには
portableなWIT型を要求する。明示的なnative WIT Moduleアダプターの
own/borrow対応も保つ。Core内だけで使える `walz:ffi-v1` はversion 1の厳密なJSONで
実際の公開関数シグネチャを記述し、externrefも検証する。権限や所有権の仮定は与えず、
externrefをportableにするものでもない。`walz:world-v1` は検査対象のworldメタデータとして予約し、
その他のcustom sectionはopaqueとして保つ。詳細は[Component構成](docs/component-composition.md)を参照。
