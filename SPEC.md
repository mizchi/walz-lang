# wlt v0 — WAT の基底と型付き拡張

## 標準 WAT との互換性

標準 WAT として parse・validate できる入力を優先し、ソースを変更せず Wasm にします。
対応範囲は固定した `wat`/`wasmparser` の既定の検証機能です。スタック形式、匿名関数、数値インデックス、線形メモリ、バイト文字列、標準の例外命令などを型付きコアの制限で拒否しません。
元の WAT にローカルの再代入や動的な呼び出しがあれば、その Wasm の意味を保ちます。`$__` の予約も拡張側だけの規約です。

標準 WAT として受理できない追加構文は、以下の型付きコアで名前・型・束縛・網羅性を検査し、標準 WAT へ展開します。
拡張を含むモジュールの本体は、現在このコアの folded expression 規約に従います。標準 WAT の任意の命令と拡張を混在させるための部分ごとの型推論は未実装です。
構文ごとの展開は固定し、利用者が eval や任意の処理を実行するマクロシステムは設けません。
`expand`/`wat` コマンドと `wlt::expand` で展開結果を確認できます。

`wlt::validate` と CLI の `check` は両経路に対応します。`wlt::check` は詳細な型を得るための型付きコア API です。
`compile` の `ir` は `Option<Module>` とし、標準 WAT がコアの外にある場合は `None` です。
その場合も Wasm の静的検証を行い、`native::types` と `types` コマンドで関数の引数・複数の戻り値・export を取得できます。式ごとの位置情報は提供しません。
詳細な解析ができない入力は `analysis.opaque=true` と `module_effects` の全項目 true を返します。空の関数一覧を「作用なし」と解釈してはいけません。
型付きコアの `module_effects` は全関数の作用の和であり、モジュールのインスタンス化そのものの作用を表すものではありません。

以下の型・不変性・構文の規則は型付き拡張のものです。

## 表現と型

プログラムは1つの `(module ...)`。宣言は WAT 形式の `type`・`func`・関数 `import`・`tag` と、追加の `closure`・`test`・`law`・`namespace` および静的な named `import` / `export` です。ファイルの宣言位置では include も使えます。
型は `i32`・`i64`・`f32`・`f64`・`(ref $T)`・`(ref null $T)`・`(option T)`・`(result T E)`・`(pair T U)`・`(own T)`・`(future T)`・`(stream T)`・`bytes`・`text`。戻り値を宣言しない式・関数は unit です。
関数の引数・戻り値、構造体のフィールド、配列の要素、クロージャの捕捉値には型が必要です。
`let` の束縛型は注釈がなければ初期値から一意に決まり、`case` の束縛型は payload の宣言から決まります。
暗黙の型変換はなく、non-null 参照から同じ型の nullable 参照への適合だけを認めます。
GC 型は名前で区別します。前方参照と再帰型を認め、出力時には1つの Wasm recursion group にまとめます。

WAT と同じ数値意味論を使います。整数演算はビット幅で折り返し、符号付き・符号なしの解釈は命令名で区別します。
整数の除算・剰余は Wasm の条件で trap します。浮動小数点も Wasm の意味に従います。
型付きコアで扱う数値命令の一覧と型制約は `src/ir.rs` の `numeric_ops!` が定義します。標準 WAT 経路の命令集合はこの表に限定しません。

名前解決・静的型検査・effect 契約・所有権の検査は `wlt::check(&str)` で独立して実行でき、検査済みの型付き IR を返します。law の証明はこの API では実行しません。
関数の引数・戻り値、呼び出し、数値命令、GC データの読み書き・構築、enum の分解、クロージャの捕捉、ループの状態更新をこの段階で検査します。
型推論は宣言済みの型に基づく単相の推論です。組込みの値型構築子は型引数ごとに具体化します。関数シグネチャの推論、暗黙の数値型変換、利用者定義の汎用関数・汎用型は未実装です。
戻り値注釈を省略した関数や result を省略した block/if/match の型は unit であり、本体から戻り値型を推論する指定ではありません。

`typing::report(&module)` は、関数ごとの params/captures の ID、戻り値型、全束縛の ID・名前・宣言位置・型・束縛の種類、全ソース式の位置・型を返します。
同名の束縛も関数内の ID で区別します。型名は `i64`・`(ref $T)`・`(ref null $T)`・`unit`・`never` のように表示します。
位置は API に渡したソース（include を使う場合は展開後）の UTF-8 byte offset の半開区間 `[start, end)` です。省略された unit 本体にはソース式の位置がなく、式の一覧には含めません。
`typing::type_at(&module, offset)` で束縛の宣言名または最も内側の式の型を取得でき、対象範囲外は `None` です。
型の問い合わせは既に解決した型を参照し、Wasm を実行しません。

## 束縛と式

識別子は `$` と ASCII 英数字・`_`・`.`・`-`。`$__` はコンパイラ用に予約します。
束縛は字句スコープで、内側の同名束縛による隠蔽を認めます。引数・捕捉宣言の重複はエラーです。

```text
(let $name [T] initial-value body)
(local.get $name)
(block [(result T)] unit-expression ... result-expression)
(if [(result T)] condition (then expression) (else expression))
(call $function argument ...)
(drop value)
(nop)
(unreachable)
```

すべて folded expression とし、1関数の本体は1つの式です。複数の処理は `block` でまとめます。
block の最後以外の式は unit でなければならず、不要な値は `drop` で明示します。
条件は `i32`、分岐の型は result と一致させます。`unreachable` と `recur` は値を返さず、任意の結果型の位置で使えます。
評価順序は左から右です。関数の定義順は呼び出しの可否に影響しません。
ローカルの再代入はありません。Wasm の `local.set` はこの言語のソースでは使えません。

`let` には `(let $x i64 (i64.const 42) body)` のように任意の型注釈を書けます。
注釈があれば初期値がその型に適合することを検査し、束縛には注釈した型を保持します。
例えば `(let $x (ref null $T) (struct.new $T ...) body)` の `$x` は nullable です。初期値の式自体の型は non-null のままです。

## GC データ

WAT 形式の struct・array を使用します。フィールド名は任意で、アクセスには名前または添字を使えます。

```wat
(type $Node (struct (field $value i64) (field $next (ref null $Node))))
(type $Cell (struct (field $value (mut i64))))
(type $Ints (array (mut i64)))
```

コア命令は `struct.new/get/set`、`array.new/new_fixed/get/set/len`、`ref.null/is_null/as_non_null/eq` です。
`array.new_fixed` の要素数は WAT と同様に明示します。`set` は宣言が `mut` の場合だけ認めます。
null、配列境界、オブジェクトの同一性、割当て失敗は Wasm の意味に従います。
nullable 参照の検査による暗黙の型絞り込みは行いません。必要なら `ref.as_non_null` を明示します。
ポインタと整数の相互変換や、生の線形メモリ操作はありません。

## Option/Result の値型

```wat
(func $make (result (option (result i64 i32))) (effects pure)
  (option.some (result i64 i32) (result.ok i64 i32 (i64.const 42))))

(match (option i64) (result i64) (option.none i64)
  (case none () (i64.const 0))
  (case some ($value) (local.get $value)))
```

型は `(option T)` と `(result T E)`。入れ子、引数・戻り値・フィールド・配列要素・明示 capture・loop 状態に使えます。
同じ型引数の組合せは1つの内部型に具体化します。型引数が異なる値を暗黙には変換しません。
関数の結果宣言で Result 値を返す場合は `(result (result T E))` と書きます。標準 WAT の複数戻り値の宣言とは文脈で区別します。

構築は `(option.none T)`、`(option.some T value)`、`(result.ok T E value)`、`(result.err T E value)` です。
型引数には unit も使えます。その側は payload を持たず、構築時の value と case の束縛を省略します。
例えば `(result.ok unit i32)`、`(option.some unit)` です。unit 自体はフィールドや引数などに保存する型としては使えません。

match の型指定には上のような型構築子を渡し、Option は none/some、Result は ok/err の全ケースを記述します。
payload の個数・型と網羅性を検査し、値がない場合も case の空の束縛列 `()` は必要です。
none と some(null)、none と some(unit)、ok(unit) と err(unit) は異なる値です。

Option/Result は意味上の値であり、格納用の GC 参照を直接操作する型ではありません。ref.eq/is_null/as_non_null や struct.get/set で格納表現を操作できません。
payload に通常の GC 参照を持つ場合、その参照の identity や可変性は引き続き payload 自身の規約に従います。
バックエンドは型ごとの非公開の GC subtype を生成しますが、これは型付き IR 上で Type::Value として区別します。

## 閉じた enum と網羅的な match

enum は `variant` 型として宣言します。ケース名は `$` を付けない ASCII 識別子で、先頭は英字または `_`、以降は英数字・`_`・`.`・`-` です。

```wat
(type $Result (variant (ok i64) (error i32)))
(type $List (variant (nil) (cons i64 (ref $List))))

(variant.new $Result ok (i64.const 42))

(match $Result (result i64) (local.get $result)
  (case ok ($value) (local.get $value))
  (case error ($code) (i64.const -1)))
```

型ごとのケース集合は閉じており、1ケース以上が必要です。重複するケース名は認めません。
各ケースは0個以上の数値・GC 参照の payload を持ちます。payload は不変で、構築時に宣言順に一度ずつ評価します。
再帰型・前方参照を認め、構築結果はその enum 型の non-null 参照になります。
構築時の個数・型を検査するので、`(ref $List)` が必要な位置に `ref.null` は渡せません。
参照先が明示的に可変なオブジェクトなら、その参照先の変更は可能です。

`match` は `(match $Type [(result R)] subject (case name ($bindings...) body) ...)` です。
subject は指定した enum 型の non-null 参照でなければならず、一度だけ評価します。
ケースの順序は自由ですが、全ケースをちょうど1回ずつ書く必要があります。未知のケース・重複・分岐漏れはエラーです。
束縛の個数は payload と一致させ、型はケースの宣言から決まります。同一 case 内の束縛名の重複はエラーです。
束縛はその case の本体内だけで使え、外側の同名束縛を隠せます。選ばれた本体だけを評価します。
各本体の型は result と適合させます。result を省略すると unit です。末尾位置の match は各 case に末尾位置を引き継ぎます。
ワイルドカード・ガード・入れ子のパターンはありません。入れ子の分解には別の match を書きます。

ソースで enum の内部を `struct.new/get/set` することや、別の enum 型として扱うことはできません。
`variant.new` は引数のないケースも毎回割り当てます。`ref.eq` は参照同一性を比較し、構造的な等値比較は提供しません。

Wasm には不変の i32 タグを持つ基底 GC struct と、ケースごとの final な subtype を出力します。
各 subtype はタグに続いて、そのケースに必要な payload だけを保持します。タグ分岐と検査付き `ref.cast` で分解します。
この基底型は Wasm の境界では sealed ではありません。外部 Wasm が渡す未知のタグ・基底型そのもの・タグと矛盾する payload 表現は、match の防御的な検査で trap します。
外部入力を事前に再帰検査する仕組みはありません。検査は各 match の実行時に行い、副作用解析では `may_trap=true` と保守的に扱います。
enum の構築も割当て失敗を含む `allocates=true`・`may_trap=true` です。不変 payload の読み取り自体は `reads` を発生させません。

## 状態を引数で渡すループ

```text
(loop $label
  (bind $state T initial-value) ...
  [(result R)]
  body)

(recur $label next-value ...)
```

初期値は外側のスコープで左から右に評価します。loop の body では state が読めます。
body が通常の値を返すとループを終了します。`recur` は最も内側のループの末尾位置でだけ使えます。
recur は各引数を左から右にすべて評価してから、すべての state を同時に更新して反復します。
個数・型は bind と一致させます。この反復は Wasm の loop/br に変換され、呼び出しスタックを増やしません。

## for マクロ

```text
(for $label
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

(try (result i64)
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
(closure $C
  (capture $captured T) ...
  (param $argument U) ...
  [(result R)]
  body)

(closure.new $C capture-value ...)
(closure.call $C closure-value argument ...)
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
(test "name" i32-expression)
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
これらの事実を型付き IR と作用の要約に保持します。現時点では独自の最適化パスは実装していません。

## 外部境界と解析

import/export は WAT の関数境界です。外部関数の実装は埋め込み側が与えます。
import には副作用を過小評価する注釈を認めず、読み書き・割当て・trap・非停止・ホスト作用を保守的に仮定します。
型付きコアの全命令を検査して IR に変換し、そこから解析と WAT 生成を分岐します。標準 WAT 経路は Wasm validator を使い、コアの解析ができない部分に純粋性を仮定しません。
解析結果の false は、その作用がこのコアのモデル内で生じないことを表します。true は可能性であり、必ず生じることを意味しません。
実行時のスタック枯渇、ホストの資源制限、デバッガによる介入などは、この副作用モデルの対象外です。

## effect 契約

```wat
(func $add (param $x i64) (param $y i64) (result i64)
  (effects pure)
  (i64.add (local.get $x) (local.get $y)))

(func $raise (effects (throws $Error))
  (throw $Error (i64.const 42)))
```

関数と closure のヘッダに `(effects ...)` を指定できます。省略時は実装から推論します。
項目は reads、writes、allocates、suspends、host、`(throws $Tag...)`、`(throws *)` です。pure は空の集合の別名で、他の項目と併記できません。
指定した集合は許可する作用の上限です。実装の作用が上限を超えた場合は、契約の位置と不足する作用を報告します。
重複、未知の項目・tag、複数の effects 宣言を拒否します。契約は本体より前に置きます。

reads/writes は可変 GC データの読み書き、allocates はソースで identity を観測できる値の生成です。
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

これらは診断用の may 解析と局所的な事実です。既存の effect 宣言・所有権の合否や、出力する Wasm は変更しません。非 escape、pure、境界確認のいずれか一つだけで identity・評価順序・trap を削除する規則も追加しません。
`proof/Flow.lean` は signed/non-negative の境界条件を BitVec で検査します。Rust の抽象解釈器や出力全体の健全性を証明するものではなく、負の添字の反例、区間と折り返し、別名・ループ・呼び出しは回帰テストでも確認します。

## WIT interface の生成

`wlt wit file.wlt` と `wit::emit(&module)` は型付きコアの公開関数から WIT を生成します。
固定の package `wlt:generated@0.1.0`、interface api、world program を使う型の契約です。この `wit` コマンド自体は実行形式を生成しません。実行用の Component は別の `component` コマンドで生成します。
内部関数とテスト関数は公開せず、引数名は p0/p1/...、export 名は元の lowercase kebab-case を保ちます。WIT のキーワードは escape し、無効な export 名は拒否します。

i32/i64 は既定の境界規約として s32/s64、f32/f64 は同名の WIT 型へ変換します。符号なし整数の意味型はまだありません。
Option/Result は再帰的に同じ WIT 型へ変換します。Result<Unit,E> は result<_,E>、Result<T,Unit> は result<T>、Result<Unit,Unit> は result です。pair は tuple に変換し、own は明示的な resource adapter を要求します。
Option<Unit> のように WIT が単位値の payload 型を必要とする場合は、1ケースの補助 enum を使います。
Future/Stream は future<T>/stream<T> に対応し、unit の payload は引数のない future/stream にします。
中断可能性が推論された関数には async を付けます。WIT に表せない GC 参照や、関数外へ出る未処理例外がある export は明示的な adapter を要求します。
WIT は任意の effect 契約を保持しないため、生成 interface だけで相手の関数の pure を仮定することはできません。

## 所有値の最小規約

```wat
(let $ticket (own.new i64 (i64.const 42))
  (match (own i64) (result i64) (move $ticket)
    (case own ($value) (local.get $value))))

(let $unused (own.new unit)
  (own.close (move $unused)))
```

`(own T)` は共有可能なデータ T を包む、複製できない値です。T は unit でもよく、その場合 own.new の値引数と case の束縛を省略します。
own.new は新しい所有値を作り、move はローカル・引数・捕捉値の所有権を渡します。所有値に local.get は使えません。move 後の再使用、所有しない値の move、所有値の drop を拒否します。
own は case own の match で消費して内容を取り出すか、own.close で明示的に消費します。暗黙の解放はありません。
現在の own.close は所有するデータの包みを消費し、Wasm の drop に展開します。OS リソースの close や Future/Stream の cancel を実装する操作ではありません。
own は payload の GC 参照を排他的にしません。既存の共有 GC 参照の別名や変更可能性はそのままです。own の内部へ別の所有値を直接包むことは認めません。

`(pair T U)` は `(pair.new T U first second)` で作り、case pair の match で分解します。unit 成分の値引数と束縛は省略します。
pair、Option、Result は、成分のどれかが所有値なら全体も所有値です。所有値を捕捉する closure も所有値になり、構築時の所有 capture と呼び出し時の環境に move を使います。
所有 closure は nullable にできず、ref.eq/is_null/as_non_null で参照を観測できません。自身を呼び出すと消費され、捕捉した所有値も本体で処理する必要があります。
共有される struct/array/参照型 variant のフィールド、例外 payload への所有値の格納は拒否します。

正常に戻るスコープでは、引数・捕捉・let・case・loop の所有値を、移動・返却・分解・close のいずれかで処理する必要があります。
初版は affine の自由な放棄を認めず、この範囲では線形の終了規約を使います。共有値は従来どおり何度でも使えます。
if/match の正常に合流する枝は、残っている所有ローカルの集合を一致させます。recur は同時更新の引数を評価した後、外側の所有集合をループ入口と一致させ、古い所有 state を残してはいけません。
状態を move して次の state に渡すことや、古い state を消費して新しい state を作ることはできます。
停止しないループの終了や、trap・OOM の際のリソース解放は証明しません。明示的な unreachable へ進む場合には、その時点の所有値を処理済みにします。

規約を小さくするため、所有値を扱う関数内の try/throw、外へ例外を出す呼び出し、所有値を受け渡す import は初版では拒否します。失敗は Result の値として返します。
borrow/lifetime、部分 move、async frame、リソース destructor は未実装です。

## 配送済みの Future と有限 Stream

```wat
(let $future (future.ready i64 (i64.const 42))
  (future.await i64 (move $future)))
```

Future/Stream は readable end に対応する opaque な所有値です。直接の match・ref 操作、共有 GC フィールドへの格納、暗黙の drop は認めません。
通常の core Wasm 出力では、Wasm GC に格納した配送済みデータを扱います。Component 出力では pending と JSPI のイベントループにも接続します。writer と言語内 producer task は未実装です。
payload T は共有できるデータに限ります。所有値や別の Future/Stream を含む payload は、終了時に黙って放棄しないよう初版では拒否します。
Result のような共有データの組合せ、GC 参照、unit は使えます。payload が GC 参照でも、その参照先の共有・変更可能性は維持します。

| 操作 | 型・意味 |
|---|---|
| `(future.ready T value)` | Future<T> を作る。unit なら value を省略 |
| `(future.await T future)` | 端点を消費して T を返す。suspends を持つ |
| `(future.close T future)` | 受け取りを放棄し、端点を消費。unit を返す |
| `(stream.empty T)` | 終端の Stream<T> を作る |
| `(stream.cons T value rest)` | value を先頭とする Stream<T> を作り、rest を消費。unit なら value を省略 |
| `(stream.next T stream)` | 端点を消費し、pair<Stream<T>,Option<T>> を返す。suspends を持つ |
| `(stream.close T stream)` | 残りの受け取りを放棄し、端点を消費。unit を返す |

構築時の引数は左から右に一度ずつ評価し、読み取りの対象式も一度だけ評価します。
await で返した Future は再利用できません。next の後は返された Stream だけを使います。
要素があれば次の端点と some(value)、終端なら終端の端点と none を返します。終端を再読しても none です。終端でも所有値が返るため、最後に close が必要です。
none と some(unit)/some(null) は区別します。Stream の終端だけでは、別の completion Future の成功や失敗は決まりません。

await/next には、中断境界として suspends を常に付けます。通常の core Wasm では配送済みデータを即時に返し、Component 出力では外部の配送も待ちます。
構築と close 自体は host/suspends を追加しません。物理的な GC 割当て・検査付き cast の trap は従来の低水準の解析に残します。
ローカル端点の close は消費だけです。Component の外部端点なら drop-readable へ変換し、ホストの dispose/iterator.return を促します。OS I/O、進行中の読み書き、producer task 全体の取消しは保証しません。

`examples/ready-delivery.wlt` は、pair<stream<i64>,future<result<unit,i32>>> を作り、40と2を読み、終端を close し、completion を確認して42を返します。
同じ所有権の契約を run-verified と Component 生成で検査できます。Component 境界では次節の Canonical ABI adapter を使います。
所有端点を raw Wasm export に渡すことは引き続き契約付き実行で拒否します。

`proof/Delivery.lean` は、一度限りの Future 配送、closed 端点の拒否、終端でも Stream の所有者が残ること、next を繰り返した結果が take/drop に一致することをモデル上で証明します。
このモデルも生成する Lean ソースに含めます。実際の Wasm の意味保存は証明の対象外として、配送順序・終端・unit・GC 参照・評価回数を実行テストで照合します。

## law と契約付き実行

```wat
(func $add (param $x i32) (param $y i32) (result i32) (effects pure)
  (i32.add (local.get $x) (local.get $y)))

(law "addition is commutative" (param $x i32) (param $y i32)
  (i32.eq (call $add (local.get $x) (local.get $y))
          (call $add (local.get $y) (local.get $x))))
```

law は名前、0個以上の param、i32 の条件式を持ちます。名前の重複を拒否し、param の全入力で条件が非0になることを要求します。
条件は既存の式で書き、別の述語言語を増やしません。law 本体の effect 上限は pure です。law はソースから生成した専用関数になり、export は持ちません。
実際の law 関数と呼び出し先の型付き IR を Lean の BitVec に変換し、bv_decide の証明を Lean のカーネルで検査します。i32/i64 の全ビットパターンを対象とし、整数の折り返し、符号付き比較、Wasm のシフト量のマスクを保ちます。
初版は i32/i64 の定数・対応する非 trap 演算、local/let/block/if、非再帰の直接 call、unit のみです。除算・剰余、再帰・ループ、浮動小数点、GC・所有値・例外などは law の意味モデルの対象外として拒否します。
証明が失敗するか、検証器が使えない場合は実行・出力を拒否します。Lean の maxHeartbeats は4,000,000で、予算内に証明できない性質も未証明として扱います。

`verify` と `run-verified` は型付きコアのみを受け付け、全 import、所有値を渡す raw Wasm export、effect 上限のない公開関数を拒否します。生成した test 関数の effect 宣言は不要です。
所有ローカルを扱う契約を JSON に抽出し、Lean の独立した検査器でも検査した後、その同じ IR から Wasm を生成・validate します。利用者が契約を弱めて実装の作用や所有操作を隠すことはできません。
law を含むモジュールでは、通常の run/build/wat/expand と CLI check もこの検査を通します。law のない通常の WAT 経路は従来のままです。
Rust API check と CLI types/ir/analyze は検査・調査用で、law の証明を実行しません。実行形式を作る compile、契約付きの verified::compile では省略できません。

`contracts` で所有権の義務、`proof` で生成した Lean ソースを確認できます。検証器は Lean 4.34.1 に固定します。
`proof/Ownership.lean` は、検査に成功した契約が宣言的な所有集合の規則を満たすこと (check_sound) と、二重消費を拒否すること (double_take_rejected) を証明します。
これは契約モデルの証明です。構文解析、Rust の型検査・effect 解析・契約抽出・BitVec への変換、Wasm への lowering、runtime/adapter は信頼する実装として残ります。コンパイラ全体の意味保存、停止性、I/O 成功を証明したものではありません。
参考にした [Bend の law/proof と quantity](https://github.com/bendlang/bend/blob/3ca3df3209ebb0fac4b80e41dac18b41442e6c58/guide/GUIDE.md) と同様に、契約、実装、検証器を分離します。Bend の依存型理論そのものは導入しません。

## 設計規約: effect と WASI 0.3.0 の対応

この節は実装の到達点と次の拡張の設計規約です。値型、effect 契約、所有権、ready-only の Future/Stream と WIT 型の生成、law と Lean による検査、解析結果の `may_suspend`、[WIT の形状契約](interfaces/async-contract.wit) は実装済みです。
外部 readable handle の pending 読み取り、Component adapter、JSPI 上の timer binding とタスクの協調的な取消しを実装しています。取消し時は進行中の read を終了させます。writer と言語側の producer task は未実装です。

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
将来の Result 伝播の糖衣は match と関数の結果返却に展開し、Wasm throw への暗黙の変換は行いません。
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
次の拡張は writer と言語側の producer task、WASI の個別 capability、利用者定義の汎用型・関数と effect の対象領域への具体化です。

### 実装した Component と JSPI の契約

`component source.wlt output.wasm` は型付きコアだけを受け付けます。公開関数の effect 上限、固定した import の名前・シグネチャ、所有権、law を検査し、Lean を通した同じ IR から Component を生成します。
公開 API は `wlt:generated/api@0.1.0`。数値、Option/Result、pair、Future/Stream を再帰的に変換します。GC 参照と own box の公開、未知の import、未処理例外は拒否します。内部では GC の循環グラフやクロージャを使えます。

内部の共有値は GC の値表現、外部の Future/Stream は readable handle を持つ非公開の GC subtype です。await/next は対象式を一度だけ評価し、端点を消費します。
外部 await は `canon future.read`、next は `canon stream.read` へ変換します。同期 Canonical ABI の read が終わるまで JSPI のスタックを中断します。Wasm の GC 参照はこのスタック上で到達可能なまま保持します。
Stream の read 結果の個数と状態を別々に解釈し、DROPPED と同時に最後の1要素が届いた場合も保存します。終端で外部 handle を drop し、再読可能なローカルの終端へ置き換えます。

境界の線形メモリは16 MiBを上限に8バイト境界で管理します。数値・variant の discriminant と payload・tuple の配置は Canonical ABI に従い、16 flat parameter を超える引数は間接渡しです。
間接引数は GC 値へ変換してから元の領域を解放します。複合戻り値は呼び出し固有の領域を返し、post-return で解放します。進行中の read 用領域は読み取り完了と lifting の後に解放します。
trap 後はインスタンス全体を破棄する前提です。破棄後のバッファ再利用や、trap 時の外部操作すべての自動取消しは保証しません。

配送済み Future と有限 Stream の前置要素は、生成した `wlt:runtime/delivery@0.1.0` の factory に渡します。factory は Promise または pull 型の AsyncIterable を返します。
producer をゲスト内で走らせないため、戻り値の消費を待ってゲスト呼び出し自体が戻れなくなる循環を作りません。Stream の外部 tail を前置要素に接続でき、close はまだ読んでいない tail にも伝播します。
unit は future/stream では payload なし、Result では省略 payload、Option/pair の要素では1ケースの `wlt-unit` enum に変換します。none と some(unit)、unit 要素と EOF を区別します。
`.wasm.json` は factory と async export の対応を記録する version=2 の manifest で、Component と対にして配布します。

信頼する event-loop import は `wlt:host/event-loop@0.1.0` の sleep/yield/defer-i64/ticks/fallible-i64 だけです。名前だけでなく型も一致する必要があります。全て host を持ち、sleep/yield は suspends も持ちます。
Future を返すだけの defer は suspends を持ちませんが、タイマーの開始という host 作用は保持します。任意の import の effect を利用者が狭める仕組みは追加していません。
端点の読み取り・close の所有契約は生成先でも維持します。現在の大域的な effect 集合には端点ごとの protocol state は含まれず、pure だけを根拠に所有操作を消去する最適化規則はありません。
`analysis` の低水準の割当て・trap 情報は既存の core lowering についての要約で、生成した ABI glue とホストの全操作の解析ではありません。

JS 実行は Node 24.21.0 / Chromium 156 の Wasm GC と JSPI、jco-transpile 0.23.0 を検証環境とします。ネイティブの Component async 関数を同期 Canonical ABI で lift/lower し、同一インスタンスの呼び出しを直列化します。JavaScript のイベントループは待機中も進行します。
`runtime/jco-compat.mjs` はこの固定バージョンの生成 JS について、unit のバッファ情報、undefined の Stream 要素、失敗した async import の確定処理を補正します。補正後の glue も信頼する実装の一部です。
Lean の Delivery モデルは ready-only のままです。pending 状態、ABI の意味保存、jco のスケジューラや Promise の完了を証明したものではありません。これらの接続は Node/Playwright で実行して照合します。

### Core＋JSPI の JS 専用 ABI

`core-js source.wlt output-directory` は、上の Component 出力と同じ型・effect・所有権・law の検査を行い、同じ IR から `core.wasm`、`memory.wasm`、`bindings.mjs`、`manifest.json` を生成します。
`runtime/build-core.mjs` は、これに共通の `core.mjs` / `host.mjs` / `tasks.mjs` と `index.js` を加えます。JS のビルド・実行に jco をロードしません。
公開できる型、信頼する import、未処理例外と配送 payload の制約は Component 出力と共通です。GC 参照や own box の公開、所有 payload・入れ子 Future の配送は引き続き拒否します。

manifest の `abi` は `wlt-core-js-v2`。Core の loader はこの値を検査し、v1 の数値 binding も受け付けます。生成 Wasm・bindings・共通ホストは同じ ABI の組として配布します。
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
現行の [Component Model の取消し規約](https://github.com/WebAssembly/component-model/blob/main/design/mvp/Concurrency.md#cancellation) は、callback ABI のタスクへ要求と確認を配送します。この実装は同期 ABI/JSPI 用の wlt の制御層であり、native `subtask.cancel` / `task.cancel` を直接使う実装ではありません。writer、言語側の producer task、ゲストから他タスクを取り消す構文は追加していません。
jco の補正には pending-copy のキャンセル接続、取消し完了後にホストの iterator.next を待ち続けない処理、ホスト解放の終了確認を含めます。これらも runtime の TCB です。

`proof/Tasks.lean` は、要求が解放を完了させるわけではないこと、read 終了前のバッファ解放拒否、全資源・ホスト解放の終了前の acknowledgment 拒否、返却後の取消しが無効であることを状態モデル上で検証します。
モデルと実装の対応、Wasm の unwind、jco の copy/table 実装、外部ホストの停止性を証明するものではありません。Node/Chromium で、開始前・開始後・read 中・返却後、遅い配送、解放の遅延・失敗、再利用とメモリ再利用を照合します。


## bytes / text と ABI

`bytes` は不変な任意のバイト列、`text` は不変な検証済み UTF-8 の値型です。
両者とも non-null で共有でき、同一性・格納参照・書込みは公開しません。
内部では packed i8 の private GC 配列を wrapper に格納し、可変な配列への書込みを構築時だけに制限します。
数値から bytes への格納は Wasm i8 と同じく下位8ビットを使います。

| 式 | 結果と規約 |
|---|---|
| `(bytes.new i32-expr...)` | `bytes`。空も可。各式は左から一度だけ評価 |
| `(bytes.from-array $Array array-expr)` | 非 null の i32 配列を不変 bytes へコピー。各要素が0..255以外なら trap。可変配列にも対応 |
| `(bytes.len bytes-expr)` | `i32` のバイト数 |
| `(bytes.get bytes-expr index)` | `i32` の 0..255。index は符号なし、範囲外は trap |
| `(bytes.slice bytes-expr start length)` | `bytes`。start <= size かつ length <= size-start。範囲外は trap |
| `(text.lit "日本語")` | `text`。ソースリテラルは UTF-8 |
| `(text.byte-len text-expr)` | `i32` の UTF-8 バイト数 |
| `(text.bytes text-expr)` | `bytes`。内容を保つ。private storage は共有可能 |
| `(text.from-bytes bytes-expr)` | `(option text)`。UTF-8 が不正なら none、正しければ some |

不正な continuation、切れた系列、overlong、surrogate、U+10FFFF より大きい値を拒否します。
NUL・BOM は通常のデータとして保存します。正規化、grapheme、コードポイント添字、比較・連結などの文字列ライブラリはまだありません。
text の byte-len は JS string.length と異なります。bytes の任意の部分列は UTF-8 とは限らないため、text への変換には常に検証を使います。

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

```wat
(module
  (namespace $math
    (public (func $add) (type $Box))
    (type $Box (struct (field $n i32)))
    (func $add (param $n i32) (result (ref $Box))
      (struct.new $Box (i32.add (local.get $n) (i32.const 2)))))
  (use $math (func $add $make) (type $Box))
  (func $main (export "main") (result i32)
    (struct.get $Box $n (call $make (i32.const 40)))))
```

`namespace` は名前の静的なスコープを作り、宣言の既定は非公開です。
`(export (func $f) (type $T) (tag $Error))` は、そのスコープの func/type/tag を静的リンク向けに公開します。`public` は互換の綴りです。closure は func と type を別々に公開できます。
`import $path (func/type/tag $member [$alias])...` は公開する宣言への静的な別名です。`use` も互換の綴りとして受理します。
名前空間をまたぐ完全修飾名 `$math.add` も公開検査を受けます。パスはルートからの絶対名です。
同一スコープの宣言と use 別名の衝突、未定義名、非公開名へのアクセスは拒否します。
名前空間は入れ子にでき、宣言と use の順序には依存しません。親スコープの名前は暗黙には取り込みません。
import した別名を export する再公開もできます。wildcard、default export、動的ロード、初期化時コードはありません。テスト名と law 名はコンパイル単位で一意にします。

型・関数・tag を完全修飾名へ解決し、通常の宣言に展開した後に型・effect・所有権を検査します。
ローカル名・捕捉名・フィールド名は書き換えません。IR と Wasm は従来と同じ単一モジュールで、実行時の namespace オブジェクトはありません。
Wasm/Component 向けの `(export "name")` は関数内の ABI export です。ルートの `(export "name" (func $local))` でも定義関数・静的 import の関数をラッパーなしに公開できます。文字列のない静的 export とは区別します。
`public (type $T)` はフィールドを含む定義を公開します。`public (opaque $T)` は型だけを公開し、表現への操作を宣言 namespace 内に制限します。フィールド単位の private は未実装です。詳細は末尾の opaque 型の節を参照してください。

ファイル分割は宣言位置の `(include "relative.wlt")` で行います。ファイルは `(module declarations...)` を含み、その宣言を展開します。
include 自体はスコープを作りません。ライブラリ側で namespace を宣言するか、呼び出し側の namespace の中に include します。
パスは include を書いたファイルからの相対位置で解決します。絶対パス、循環、深さ64以上、展開後16 MiBを超える入力は拒否します。
同一ファイルを自動で一度だけにする処理はなく、重複する宣言はエラーです。
CLI とテストランナーは include を展開し、通常の型エラーは元ファイルの位置へ戻します。
ライブラリ API は `source::load(path)` と `check(loaded.text())` / `compile(loaded.text())` を分け、文字列を受け取る check/compile 自体はファイルを読みません。
IR の Span は展開後の text に対する範囲です。診断は `loaded.diagnostic(error)` で元の位置へ変換できます。

ファイルからの `(import "relative.wlt" (func $f $alias) (type $T) (tag $E))` は選んだ公開名だけを束縛します。
各ファイルは独立したスコープを持ち、canonical path ごとに一度だけ読み込み、型と定義を共有します。
ファイル内 namespace の公開パス（例 `$string.u64`）も指定できます。親や依存ファイルの名前は暗黙に取り込みません。
全ソースの検査後、ABI export・test・law から到達する関数・型・tag だけを生成します。未使用のホスト import と capability も除去します。
IR と解析 API は削除前の全ソースを保持します。生成 helper の依存とプロファイラの ABI も同じ到達性投影を使います。
標準 WAT は書き換えず、この除去は型付きのリンク経路に限ります。詳細は [モジュールと DCE](docs/static-modules.md) を参照してください。


メモリ管理は Wasm GC の回収と ABI/外部端点の明示的な寿命を分けます。境界の規約・再現した保持と検査範囲は [MEMORY.md](MEMORY.md) を参照してください。JS 側の Stream の EOF は、そのホスト reader を閉じる契機とします。ソース上の Stream の EOF 後に close が必要な規則は維持します。

## 値だけを渡す text I/O capability

`wlt:host/text-io@0.1.0` は固定した信頼境界です。`read-lines` は `stream<result<text,i32>>`、`write-line(text)` は `future<result<unit,i32>>` を返します。
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
`examples/io/text-filter.wlt` は空行を除いて出力し、成功時は行数を返します。stdout に行数や manifest を混ぜません。
終了コードは成功0、Result.err または実行失敗1、SIGINT による TaskCancelledError は130です。SIGINT 後も cleanup の終了を待ち、読み書き途中のファイルを閉じます。
同じ path または同じファイルを指す入力・出力は、書き込み前に拒否します。出力ファイルは明示的に選んだものを作成・上書きし、空の出力でも空ファイルにします。
Node の handle 管理は runtime/node-io.mjs に分離します。通常完了で借用した stdout を閉じることはせず、factory で開いたファイルは end/close まで待ちます。Node 版の取消しでは入出力の stream を閉じるため、一回の実行スコープへ接続します。

この WIT と接続コードは wlt 固有であり、wasi:* package の実装や汎用の WIT import generator ではありません。既存の Lean モデルを拡張した意味保存の証明は追加しておらず、Rust/Node/Chromium の実行検査で接続を照合します。

## 文字列ライブラリと配列のコピー

`bytes.from-array` の引数は指定した配列型の非 null 参照で、式を一度だけ評価する。
元配列と格納域を共有せず、元配列を後から変更しても bytes/text の内容は変わらない。
可変配列を読む場合は reads を持ち、memory 解析にも対象の要素読み取りを残す。返す値から元配列への参照はない。
新しい bytes 自身に観測可能な identity はないので、格納用の物理的な割当てと semantic allocates は区別する。

ルートの宣言位置で `(include "stdlib/text.wlt")` を使う。include のパスは呼出し元からの相対位置に合わせる。
`$buffer.concat(bytes,bytes)`、`$string.concat(text,text)`、`$string.u64(i64)`、`$string.s64(i64)`、`$string.json(text)` を提供する。
u64 は64 bitのビットパターンを符号なし十進数へ、s64 は符号付き十進数へ変換する。
json は引用符を含む JSON string を返し、quote/backslash/制御文字をエスケープする。
これらは通常の wlt の関数で、ローカルな GC 配列の構築・変更を含むため reads/writes/allocates の上限を持つ。
可変な作業配列と、公開する不変の bytes/text はコピーで分離する。concat の計算量は入力長の合計に比例する。

検索・分割・上限付き整数解析と text/JSON builder の API は [stdlib/README.md](stdlib/README.md) に定義する。
検索位置は符号なしの UTF-8 byte 単位であり、text の検索開始位置が文字の途中なら none を返す。
整数解析は乗算前の上限検査を行い、空・不正範囲・桁あふれを option.none として扱う。
builder は通常の不透明な可変 GC 型で、finish が断片への参照を外して再利用可能な空の状態に戻す。
JSON object builder はフィールド名・値のエスケープと区切りを管理し、入れ子の child は finish して取り込む。
これらは言語の組込み型や構文を増やさず、既存の型・effect・opaque 検査に従う。Lean による意味保存の証明は追加していない。

## Result と Stream の限定マクロ

```wat
(result.let ($value) (result T E) U subject body)
(stream.fold-result T S E source initial ($state $item) body)
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
(namespace $counter
  (public (opaque $State) (func $new) (func $get))
  (type $State (struct (field $n (mut i32))))
  (func $new (result (ref $State)) (struct.new $State (i32.const 0)))
  (func $get (param $s (ref $State)) (result i32)
    (struct.get $State $n (local.get $s))))
```

外部では nominal な型として参照・受渡しできる。通常の `(use $counter (type $State $Alias))` も使える。
構築、フィールド／要素の読書き、variant の match、配列長取得と bytes.from-array は、宣言した namespace の中だけで認める。
別名経由と、型が推論された参照経由にも同じ検査を行う。ref.eq、nullability と GC の意味は既存の規則を使う。
検査は型付き IR の操作と元の宣言スコープを照合し、追加の実行時ラッパーや ABI は作らない。
フィールドごとの個別公開と friend は初版に含まない。

## ソースの整形と CLI の診断

`wlt fmt file.wlt` はそのファイルだけを整形し、`fmt-check` は未整形なら終了コード1を返す。include の参照先へ自動的に書き込まない。
文字列のエスケープ、行コメント、入れ子のブロックコメントを保持し、トークン間の空白だけを変更する。
括弧・文字列・コメントが閉じていなければ書き込まない。閉じていない括弧は開始位置を報告する。
formatter は型検査の代わりではなく、標準 WAT のスタック構文にも使える。
text CLI の Result err は `program error <code>` と表示し、アプリケーションのエラーも含む。
EOF で完了した正常な main、Result の err、SIGINT の終了コードは従来どおり0、1、130。

## 透明な型 alias と局所的な型引数の省略

`(type.alias $Name T)` は型の出現箇所に展開する透明な略記です。型名の名前空間を使い、GC の type / closure と同名にはできません。
前方参照を許可し、循環・深さ256超・展開ノード100万超を拒否します。未使用の target も既存の型・所有性規則で検査します。
alias 自体を実行用 IR の nominal type にせず、未使用 alias の型を実行用 Wasm に追加しません。
namespace 内の `public (type $Name)` と `use` に対応します。alias に opaque は指定できず、target の opaque 制限は展開後も適用します。
型の位置では `$Name` と書きます。heap type ID を要求する `(ref $Name)` や `struct.get $Name` 等には使えません。
unit の alias は payload の位置に限定し、通常の関数引数やフィールドへ unit を導入しません。

次の省略形を既存の明示形に追加します。

```text
(result.ok [value])                 ;; expected result<T,E>
(result.err [error])                ;; expected result<T,E>
(pair.new [first] [second])         ;; expected pair<T,U>、unit成分は省略
(option.none) / (option.some value)  ;; expected option<T>
(own.new [value])                   ;; expected own<T>
(match subject (case variant ($names...) body)...)
(let ($first $second) subject body)
(result.let ($value) subject body)
(stream.fold-result source initial ($state $item) body)
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
