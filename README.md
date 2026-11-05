# WLT

[mizchi/wlt-lang](https://github.com/mizchi/wlt-lang) は Wasmtime の作業ディレクトリ内の試作から独立した言語リポジトリです。
旧名 gcwat からの変更点と過去の評価記録の扱いは [MIGRATION.md](MIGRATION.md) にまとめています。

Wasm GC と静的解析を前提に、標準 WAT を基底として型付きマクロを追加する小さな言語の試作です。
標準 WAT は書き換えず受け付け、拡張側には不変の `let`、loop/for、明示 capture、enum/match、Wasm 例外の try/catch を用意します。`if` の結果値は標準 Wasm の意味を使います。
構造体・配列・クロージャ環境は Wasm GC のオブジェクトです。Component 境界の変換用バッファだけを、専用の線形メモリアロケータで管理します。

`bytes` / `text` は不変な値で、Component の `list<u8>` / `string` と相互変換できます。
相対パスの named import/export で関数・型・tag を選択し、単一 Wasm に静的リンクできます。[モジュールと DCE](docs/static-modules.md) に規約をまとめています。
[文法チートシート](CHEATSHEET.md) に型・式・所有値・固定マクロと実行手順をまとめています。
[初見評価の記録](docs/grammar-eval/REPORT.md) は、在庫台帳の読解と変更を固定ルーブリックで確かめたものです。
[可読性の評価と改善](docs/readability/REPORT.md) では、型の反復・入れ子・配置・変更の追いやすさを別に評価しています。
[evals/](evals/README.md) に評価を繰り返す課題・基準・スナップショット作成・実行検証をまとめています。`just eval-prepare ID` で新しい評価回を作れます。
型 alias、期待型による型引数の省略、pair の分解 let を使う [行数付き在庫台帳](examples/apps/stock-ledger/main-counted.wlt) もあります。
[仕様と構文](SPEC.md#bytes--text-と-abi)、[bytes/text の例](examples/bytes-text.wlt)、[モジュールの例](examples/modules.wlt) を参照してください。

```sh
git clone https://github.com/mizchi/wlt-lang.git
cd wlt-lang
just                         # クロージャの例 → 42
just run examples/counter.wlt
just run examples/collections.wlt
just run examples/list.wlt
just run examples/variants.wlt
just run examples/ast-interpreter.wlt
just run examples/bytes-text.wlt
just run examples/modules.wlt
just wat                     # 通常の .wat へ変換
just expand examples/exceptions.wlt  # try/catch などの展開を確認
just build                   # target/program.wasm を生成
just analyze                 # 呼び出しグラフと副作用を JSON で表示
just ir                      # 名前解決・型検査済み IR を JSON で表示
just validate examples/result.wlt  # ソースの静的型検査
just types examples/result.wlt     # 式と束縛の型を JSON で表示
just wit examples/value-types.wlt  # 公開関数の WIT interface を生成
just run-verified              # 所有権と law の証明を通して → 42
just run-verified examples/ready-delivery.wlt # Future/Stream の配送済みデータ → 42
just verify                   # 契約検査のみ。実行しない
just contracts                # 所有権の義務を JSON で表示
just proof                    # 型付き IR から生成した Lean ソース
just proof-check              # 所有権モデルの定理と反例を検証
just test
just test-language            # wlt で書いたテストを実行
just list-tests               # テスト一覧。実行はしない
just wit-check                # async 型の WIT 契約を往復検証
just check
just samples                 # 実行サンプルと期待値を一覧で確認
just probes                  # 未対応機能を再現する検査
```

Rust 1.97 以降と just を使用します。実行と実行テストには Wasm GC 対応の `wasmtime`、WIT のテストには `wasm-tools` が PATH 上に必要です。
契約付き実行・law を含むモジュール・Rust の検証テストには、elan 経由の `lean +v4.34.1` も必要です。
`check`・`wat`・`build`・静的解析には Wasmtime は不要です。law を含まない通常のコンパイルでは Lean も不要です。just を使わず `cargo run --locked -- run examples/closure.wlt` としても実行できます。
生成した Wasm は `wasmtime run -W gc=y,function-references=y,exceptions=y --invoke main target/program.wasm` で実行できます。
ランチャーでは GC・function references・exceptions を明示的に有効にします。

サンプルは [examples/catalog.json](examples/catalog.json) で管理しています。
30本の実行例に、ソート、行列積、AST 評価、for、例外、値型、所有権、law、Future/Stream、標準 Wasm の数値命令・末尾呼び出しなどを含めています。
追加の31入力、意図した実行時失敗2本、未対応機能3本もテストします。
実装できた範囲と不足機能の考察は [ASSESSMENT.md](ASSESSMENT.md) を参照してください。
残る機能と実装順は [TODO.md](TODO.md) で管理しています。項目6.1–6.3と、bytes/text・静的モジュールの初版は実装済みです。

```wat
(module
  (closure $Adder
    (capture $base i64)
    (param $x i64)
    (result i64)
    (i64.add (local.get $base) (local.get $x)))

  (func $main (export "main") (result i64)
    (let $add40 (closure.new $Adder (i64.const 40))
      (closure.call $Adder (local.get $add40) (i64.const 2)))))
```

クロージャは宣言ごとの固有型を持ちます。捕捉値を引数で渡して構築し、呼び出し先は型から静的に決まります。
例えば `$Adder` は、捕捉値を保持する GC struct と、環境を第1引数に取る通常の Wasm 関数へ変換されます。
可変状態を捕捉する場合は `counter.wlt` のように、`mut` フィールドを持つオブジェクトへの参照を渡します。

enum は `(type $Result (variant (ok i64) (error i32)))` と宣言し、`(variant.new $Result ok (i64.const 42))` で構築します。

```wat
(match $Result (result i64) (local.get $result)
  (case ok ($value) (local.get $value))
  (case error ($code) (i64.const -1)))
```

payload の型と個数、case の網羅性と重複をコンパイル時に検査します。再帰 enum と、case 内の明示 capture・末尾 `recur` も使えます。
`examples/ast-interpreter.wlt` はこの形式で実装しています。`examples/result.wlt` は組込みの Result 値型を使います。

```wat
(func $answer (result (option i64)) (effects pure)
  (option.some i64 (i64.const 42)))
```

`(option T)`・`(result T E)` は型引数ごとに具体化します。入れ子、unit payload、配列・フィールド・明示 capture に対応します。
これらの格納用 GC オブジェクトの identity はソースで比較できません。payload 自身の GC 参照の性質は保持します。
`(effects ...)` は reads/writes/allocates/suspends/host と、関数外へ出る throws の上限を検査します。
pure な関数でも値のための物理的な割当てや trap はあり得るため、従来の低水準の解析情報も別に残します。

`(pair T U)` も値型として使え、WIT では tuple に対応します。

## 所有権と証明を通して実行する

```wat
(let $ticket (own.new i64 (i64.const 42))
  (match (own i64) (result i64) (move $ticket)
    (case own ($value) (local.get $value))))
```

`(own T)` は複製できない包みです。move 後の再利用、暗黙の放棄、共有 GC フィールドへの格納を拒否します。
Option/Result/pair と捕捉するクロージャにも所有性が伝播します。使わない包みは `(own.close (move $ticket))` で明示的に消費します。
内容を match で取り出すことも消費です。own は payload の GC 参照を排他的にする型ではなく、own.close は外部リソースの終了処理をまだ実装していません。
borrow/lifetime は設けず、操作に渡した所有値は戻り値の一部として受け直す方針です。[ownership.wlt](examples/ownership.wlt) に組と once-call クロージャの例があります。

```wat
(law "adding zero preserves the value" (param $x i32)
  (i32.eq (call $add (local.get $x) (i32.const 0)) (local.get $x)))
```

law は i32/i64 の全入力で条件が非0になることを要求し、実際の関数本体から生成した BitVec の証明を Lean で検査します。
整数の折り返し・符号・Wasm のシフト量のマスクを含めます。初版の law は純粋な整数式と非再帰の直接呼び出しに限定します。
関数本体を変更して law が壊れると、通常の run/build も拒否します。検証器の不在や、未対応の意味論もエラーになります。
[verified-laws.wlt](examples/verified-laws.wlt) は加算の law と所有値を組み合わせた実行例です。

`run-verified` は所有権の契約も独立した Lean の検査器へ渡し、通った同じ IR から Wasm を生成します。
このモードでは未解析の WAT、import、所有値を渡す raw export、effect 上限のない公開関数を拒否します。通常の WAT の受付は維持します。
Rust API `check` と調査用の types/ir/analyze は law を証明しません。CLI check と実行形式の生成は、law があれば証明を検査します。
形式化した範囲と信頼する実装は [SPEC.md](SPEC.md#law-と契約付き実行)・[proof/README.md](proof/README.md) に記載しています。コンパイラ全体の意味保存や、停止性の証明は含みません。

## 配送済みの Future と有限 Stream

```wat
(let $future (future.ready i64 (i64.const 42))
  (future.await i64 (move $future)))
```

`(future T)` と `(stream T)` は opaque な所有値です。ready/empty/cons で構築し、await/next/close だけで扱います。
next は `(pair (stream T) (option T))` を返すので、次回は返された Stream を使います。終端の none でも端点が返り、最後に明示 close が必要です。
payload は共有できるデータに限定し、unit、Result、GC 参照も扱えます。Future/Stream 自体や own 値を payload にすることは初版では拒否します。
await/next は suspends を持ち、呼び出し元へ伝播します。WIT の future/stream 型と読み取り関数の async にも対応します。

[ready-delivery.wlt](examples/ready-delivery.wlt) は stream の40と2を合計し、別の completion Future の Result も確認して42を返します。
通常の core Wasm 出力では配送済みデータを GC に保持し、読み取りは即時に返ります。
Component 出力では外部の readable handle も保持でき、await/next は JSPI を通じて Promise/AsyncIterable の配送を待ちます。
close は端点を消費し、外部端点なら Canonical ABI の drop-readable を実行します。固定したホスト実装では不要になったタイマーを解放します。
JS ランタイムからの task cancellation と、それに伴う進行中の read の cancel に対応します。writer と言語側の producer task は未実装です。

## Component ABI と JavaScript のイベントループ

```sh
pnpm install --frozen-lockfile
just component               # target/component.wasm と .wasm.json を生成
just build-js                # jco で JSPI 用の JS と core Wasm に変換
just run-js                  # タイマーを待ち、GC 内の値を使って → 42
node runtime/run.mjs target/event-loop sumTicks 5 1  # Stream の総和 → 10
just test-js
pnpm exec playwright install chromium
just test-e2e
```

Rust/Lean に加え Node.js 24 以降と pnpm を使います。Node 24.21.0、pnpm 12.11.2、jco-transpile 0.23.0、Playwright 1.64.1 の Chromium 156 で確認しています。
実行環境には Wasm GC、例外機構（try_table）、[JSPI](https://v8.dev/blog/jspi) の `WebAssembly.Suspending` / `WebAssembly.promising` が必要です。
[jco](https://bytecodealliance.github.io/jco/transpiling.html) が Canonical ABI と JSPI の橋渡しを行い、待機中も JavaScript のタイマーが進みます。同じ Component インスタンスへの呼び出しは順番に実行します。

[event-loop.wlt](tests/component-fixtures/event-loop.wlt) は、GC 参照を待機中も保持する main、非同期 Stream の総和、completion Result の分岐、pending Future の close を実装しています。
ホストの固定契約は `wlt:host/event-loop@0.1.0` です。

| import | 型 | 作用 |
|---|---|---|
| sleep | `(i32) -> unit` | host, suspends |
| yield | `() -> unit` | host, suspends |
| defer-i64 | `(i32, i64) -> future<i64>` | host |
| ticks | `(i32, i32) -> stream<i64>` | host |
| fallible-i64 | `(i32, i32) -> future<result<i64,i32>>` | host |

時間はミリ秒です。ticks は要素数・間隔を受け取り、0 から順に配送します。fallible-i64 は code=0 なら ok(42)、それ以外なら err(code) を配送します。
公開関数には effects 契約が必要です。Component の生成でも、同じ IR の所有権契約と law を Lean で検査します。未知の import、境界の GC 参照や own、外へ漏れる例外は拒否します。

内部値と ABI バッファを分離し、数値、Option、Result、pair、Future、Stream を変換します。Future/Stream は Component Model 自体の型と readable handle です。
バッファは呼び出しごとに確保し、戻り値は post-return、引数や read は変換終了後に解放します。アリーナ上限は16 MiBです。
`wlt:runtime/delivery@0.1.0` は ready 値や Stream の前置要素を JS の配送端点へ移すための内部 import です。型と factory 名はモジュールごとに生成するため、`.wasm.json` と対応する JS ホストを同時に配布します。
`wlt wit` は公開 API の論理的な形状を出力し、`wasm-tools component wit target/component.wasm` は内部 import を含む実際の Component 契約を出力します。

生成する JS には、固定した jco バージョンの unit/option.none の配送と、失敗した import の Promise が確定しない問題への局所的な補正を適用します。内容は [jco-compat.mjs](runtime/jco-compat.mjs) に集約し、実行テストで両方向の配送と失敗を確認します。
アプリケーションの失敗は Result で返します。不正なタイマー引数は trap となり、そのインスタンスは再利用できません。
現段階では独自の timer binding を実装しており、wasi:* のファイル・ネットワーク API や WASI 0.3.0 一式の実装ではありません。

### 軽量な Core＋JSPI 出力

```sh
just build-core-js                  # target/core-event-loop に生成
just run-js target/core-event-loop  # → 42
just test-core-js
just memory-core-js
```

JS 向けには、[taraba](https://github.com/mizchi/taraba) の構成を参考にした専用の出力も使えます。
`runtime/build-core.mjs` は Component transpiler をロードせず、2つの core Wasm と、小さな共通ホスト・生成済みの型変換を出力します。
`instantiate()` の `api` / `start` と event-loop binding は上の経路と共通です。同期関数の同期性、Future/Stream の所有権、取消しの終了確認も維持します。
GC 値と停止中のスタックは Wasm エンジンが管理し、ホストが手動管理する対象は一時バッファと外部端点に限定します。

この経路の ABI は `wlt-core-js-v2` です。標準 Component との相互運用には `just component` / `just build-js` を使います。
公開 API と配送 payload の具体的な型から codec を生成し、実行時に型スキーマを解釈する処理は持ちません。
API の制約・内部 ABI は [SPEC.md](SPEC.md#corejspi-の-js-専用-abi)、サイズ・メモリの比較は [MEMORY.md](MEMORY.md) を参照してください。

`just profile before` / `just profile after` で行フィルタの guest profile と実行メトリクスを保存し、
`just profile-compare before after` で保存した成果物を交互に再測定できます。
ABI 割当てを約19%減らした変更、UTF-8 検証のチューニングと測定条件は [PROFILE.md](PROFILE.md) に記録しています。

少し大きい実用例として、[ログ集計 CLI](examples/apps/log-stats/README.md) を追加しました。
静的モジュールと文字列ライブラリで TSV の解析、GC 上の集計、整数の文字列化、JSON の1行出力を実装しています。
Result / Stream のマクロで所有権を保つ反復を記述し、集計状態は opaque な型として公開します。
`just build-log-stats` の後、`just log-stats input.tsv` で実行、`just test-log-stats` で検査できます。
`just profile-log-stats` は、共有する計算部分の guest profile と、実際の Core＋JSPI 経路の実行時間を保存します。

改善後の機能を別の処理で確認する [在庫台帳 CLI](examples/apps/stock-ledger/README.md) もあります。
`just build-stock-ledger` → `just stock-ledger` で、Unicode の品目名と数量から JSON Lines を出力します。
手書き版とマクロ版を同じ操作列で照合し、`just audit-stock-ledger` で記述量・生成 IR・実行時間を比較できます。

### UTF-8 行フィルタと I/O capability

```sh
just build-text
printf '日本語🦀\n\nlast\n' | just text-filter
just text-file input.txt output.txt
just test-io
```

[行フィルタ](examples/io/text-filter.wlt) は、空行を除いて入力を出力し、出力した行数を `result<i32,i32>` で返します。
入力は LF / CRLF 区切りで、末尾の改行なし、チャンク途中で分割された UTF-8 に対応します。出力は各行に LF を付けます。
CLI の stdout は処理したデータだけで、終了コードは成功0、I/O の失敗1、SIGINT による取消し130です。同じファイルを入力と出力へ指定することは拒否します。

固定の契約は [text-io.wit](interfaces/text-io.wit) の `wlt:host/text-io@0.1.0` です。

| import | 型 | 必須 capability |
|---|---|---|
| read-lines | `() -> stream<result<text,i32>>` | text-input |
| write-line | `(text) -> future<result<unit,i32>>` | text-output |

両方とも host 作用を持ちます。呼び出しは端点を返すだけで、読み取り・完了待ちが suspends を持ちます。
コンパイラは import の名前・シグネチャ・effect を検査し、manifest の `imports` に必須 capability と型を記録します。
埋め込みでは `instantiate({imports: {'wlt:host/text-io@0.1.0': io}})` で明示的に host を渡します。必要な関数がない場合はインスタンスの生成を拒否します。
`runtime/text-io.mjs` の `createTextIO({input,write})` は、byte iterable と `write(bytes,AbortSignal)` を既存の値型に接続します。
`runtime/node-io.mjs` は Node のストリームとファイルの開閉、`runtime/text-cli.mjs` は CLI を担当します。
これらの I/O 実装は通常の Core 出力へ自動では組み込みません。ファイルパスや標準入出力を選ぶ権限は host 側にあります。

I/O エラーコードは UTF-8 不正1、読み取り失敗2、書き込み失敗3、行長上限4、閉じた入力または書き込み中止5です。
入力は一度だけ消費でき、行の上限は既定1 MiBです。JS の helper では `maxLineBytes` を最大8 MiBまで設定できます。
入力を close すると provider の return を待ち、出力の取消しは AbortSignal を通知して sink の終了を待ちます。Node の CLI はファイルと未完了のストリームも閉じます。
この例は Node/JS の capability binding であり、WASI のファイル・ネットワークパッケージの実装は今後の範囲です。

### タスクの取消し

```js
import {instantiate, TaskCancelledError} from './target/event-loop/index.js';
const {api, start} = await instantiate();
const controller = new AbortController();
const task = start('main', [60_000], {signal: controller.signal});
setTimeout(() => controller.abort('timeout'), 10);
try {
  console.log(await task.result);
} catch (error) {
  if (!(error instanceof TaskCancelledError)) throw error;
}
const outcome = await task.cancel(); // 要求を再送しても、同じ終了確認を返す
console.log(outcome);                // {status: 'cancelled', started: true/false}
console.log(await api.main(0));      // 取消し後も同じインスタンスで 42n
```

`start` の名前は JS の export 名です（例: `sumTicks`、`catchAll`）。引数は配列で渡します。
`task.result` は通常の値を返す Promise で、取消し後は TaskCancelledError を返し、cause に最初の取消し理由を残します。Result の err や Wasm のアプリケーション例外へは変換しません。
`cancel(reason)` は要求と終了確認を分けます。返す Promise は、ゲストのスタック、所有する readable end、ABI バッファ、ホストの非同期 dispose/iterator.return の処理が終わるまで確定しません。
開始前の取消しではゲストを実行せず、引数の所有権も移しません。開始後は待機点で取消しを確認し、内部の制御用例外で巻き戻します。ソースの catch_all はこの取消しを捕捉できません。
終了結果は status=returned/cancelled/failed と、開始済みかを示す started です。既に値を返したタスクと、呼び出し元に渡した Future/Stream は、後から cancel しても取り消しません。

取消しは協調的です。CPU だけを使う処理の強制停止、既に行った変更の巻き戻し、言語側の producer task の停止は行いません。
カスタムホストが進行中の操作を終了できない場合は、終了確認も待ち続けます。解放処理が失敗した場合は failed と AggregateError を返し、そのインスタンスの再利用を拒否します。
直接の `api` 呼び出しでは、async export は順番に実行します。同期 export は同期のままで、他のタスクの実行・解放中なら busy エラーを返します。同期 export を待ち行列に入れる場合も `start` を使います。
Component 経由の ABI と manifest は version=2 です。旧出力は `just build-js` で再生成してください。
Lean のモデルでは一度限りの受け取り、配送順序、終端・close を検証し、実コードの Wasm 実行と照合します。

仕様は [SPEC.md](SPEC.md) にまとめています。型付きコアは単一スレッド、単相型、単一戻り値の核です。
型付き拡張では暗黙の捕捉・型変換を使わず、名前・型・呼び出し先・副作用を追います。
標準 WAT 経路では元の命令・型規則を維持します。任意の WAT と拡張を混在させる構文解析はまだなく、拡張を含むモジュールは型付きコアの folded expression 規約に従います。

```wat
(tag $Error (param i64))
(try (result i64)
  (do (throw $Error (i64.const 42)))
  (catch $Error ($value) (local.get $value)))
```

try/catch は `try_table` へ展開し、trap は捕捉しません。for は bind/while/step を持ち、loop/if/br へ展開します。
[exceptions.wlt](examples/exceptions.wlt) と [for-loop.wlt](examples/for-loop.wlt) に実行例があります。

## 型検査と型の取得

複数の機能を組み合わせた例として、[依存グラフのワークフロープランナー](examples/apps/workflow/README.md)があります。
GC グラフ・状態 enum・複数ワーカーへの割当て・循環検出・非同期 I/O を実装し、独立した参照実装と両バックエンドで照合しています。

関数の引数・戻り値などの注釈を境界にして、各式の型、`let` の束縛型、`case` の payload の型を求めます。
`let` にも `(let $answer i64 (i64.const 42) body)` と任意の型注釈を書けます。暗黙の数値型変換は行いません。
型エラーは位置とソース上の型名で表示し、例えば `type mismatch: expected i64, got i32` となります。
引数数、名前候補、型の期待値の根拠、先行する move、effect の原因、opaque の所有 namespace も診断します。
Rust API の `Error::diagnostic()` では分類・主範囲・関連位置・補足・修正方針を取得できます。[診断のデータ契約](docs/diagnostics.md)を参照してください。

```sh
cargo run --locked -- check examples/ast-interpreter.wlt
cargo run --locked -- types examples/ast-interpreter.wlt
```

型付きコアの `types`・`ir`・`analyze` は名前解決済みの IR を使います。CLI の `check` は標準 WAT も検証します。
`types` は関数の戻り値型、params/captures のローカル ID、全束縛の型と宣言位置、全式の型とソース範囲を返します。
ローカル ID により同名の束縛を区別します。ソース範囲は UTF-8 byte offset の半開区間です。

Rust API では `wlt::check(source)` が型付き IR を返し、`wlt::typing::report(&module)` で型の一覧を取得できます。
`wlt::typing::type_at(&module, offset)` は束縛宣言または最も内側の式の型を返します。
取得した型は `module.type_name(ty)` で `(ref $Expr)` などの表記にできます。
関数シグネチャは明示したままで、省略した result は unit です。全プログラムの型注釈を省略する推論は行いません。
`wlt::validate(source)` は標準 WAT と拡張の両方に対応します。標準 WAT がコアの外にある場合、`types` は検証済み Wasm の関数シグネチャを表示し、式ごとの型は取得できません。
この場合 `Compilation.ir` は None、解析は `opaque=true` でモジュールの全作用を保守的に true とします。

## 言語内のテスト

モジュール直下に名前と `i32` の条件式を置きます。0 が失敗、それ以外が成功です。

```wat
(module
  (func $double (param $n i64) (result i64)
    (i64.mul (local.get $n) (i64.const 2)))

  (test "double returns 42"
    (i64.eq (call $double (i64.const 21)) (i64.const 42))))
```

```sh
just test-language
just test-language tests/programs GC
just list-tests tests/programs
cargo run --locked -- test tests/programs --filter closure
cargo run --locked -- test tests/programs/core.test.wlt --timeout-ms 1000
```

`wlt test` は既定で `tests/` 配下の `*.test.wlt` を再帰探索します。ファイルを明示した場合は名前を問いません。
複数のパスを渡せ、同じファイルは1回だけ実行します。ディレクトリ探索ではシンボリックリンクをたどりません。
`--filter` はテスト名の部分一致、`--list` は型検査と一覧表示のみです。絞り込み前にモジュール全体を型検査します。
失敗、trap、コンパイルエラーがあっても他のテスト・ファイルを続けます。0件の場合も成功扱いにしません。
終了コードは成功が0、テスト失敗・実行環境やソースのエラーが1、引数の誤りが2です。

各テストは新しい Wasmtime プロセスとインスタンスで実行します。既定の制限時間は起動・コンパイル込みで1本5秒です。
`--timeout-ms` で変更できます。タイムアウト時はプロセスを終了し、次のテストを実行します。実行には PATH 上の Wasmtime が必要です。
テスト失敗時の実行時診断は最大64 KiBを表示します。import の実装は通常の run と同様にランナーでは提供しません。

テスト本体も通常の型付き IR へ変換し、Wasm GC 上で実行します。暗黙の capture・共有可変状態・動的な関数レジストリは追加しません。
GC の割当て、参照同一性、明示した変更、trap、評価順序は通常のコードと同じです。型・副作用の検査と定義域は [SPEC.md](SPEC.md) を参照してください。
[core.test.wlt](tests/programs/core.test.wlt)・[gc.test.wlt](tests/programs/gc.test.wlt)・[macros.test.wlt](tests/programs/macros.test.wlt)・[values.test.wlt](tests/programs/values.test.wlt)・[ownership.test.wlt](tests/programs/ownership.test.wlt)・[delivery.test.wlt](tests/programs/delivery.test.wlt) の計21本は、`just test` からも検証します。
Rust API は `wlt::runner::run(paths, options)` で、各テストの結果・ファイル単位のエラーを構造化して取得できます。

## 静的解析

解析結果は関数とクロージャごとに次の可能性を報告します。

| 項目 | 意味 |
|---|---|
| `reads` | 明示的に可変な GC フィールド・要素を読む |
| `writes` | GC フィールド・要素を書き換える |
| `allocates` | GC オブジェクトを割り当てる |
| `may_trap` | null、境界、整数除算、割当て失敗などで trap する可能性 |
| `may_throw` | Wasm 例外を送出する可能性。関数内で捕捉するものも含む |
| `may_suspend` | 論理的に中断し、他の処理が介在する可能性。未知の外部呼び出しは保守的に true |
| `may_diverge` | ループ・再帰・外部呼び出しにより停止しない可能性 |
| `host` | 外部のホスト関数を呼ぶ |

型付きコアの呼び出し先は静的に決まるので、再帰を含む呼び出しグラフに沿って固定点まで副作用を伝播できます。
不変フィールドの読み取りは `reads` に含めず、nullable な参照なら `may_trap` を付けます。
`match` は外部 Wasm の不正な enum 表現に備える実行時検査を持つため、`may_trap` を保守的に付けます。
未知の外部関数は全項目を保守的に true とします。固定した event-loop binding は型と作用を検査し、host と必要な suspends を伝播します。ホストへの import を実行する場合は、埋め込み側で実装を与えてください。
CLI の `run` は import のない `main` を実行する最小ランチャーです。

この解析は安全側の近似です。ループや再帰の停止性、添字範囲、割当て失敗が起こらないことは証明しません。
解析情報を使う独自の最適化パスはまだありません。出力後のコード生成・最適化は Wasmtime/Cranelift が担当します。

### 読み書きの対象と、分岐から得られる事実

`just analyze examples/higher-order.wlt` は、従来の副作用フラグに加えて、各関数の memory と facts を出力します。
map では memory.reads が入力の parameter[0]、memory.writes が map 内の allocation を指します。
出力配列の長さが入力と等しいことも保持し、読み取りと書き込みの両方を bounds_proven=true と確認できます。

| 出力 | 内容 |
|---|---|
| memory.reads / writes | 可変な GC フィールド・要素を読む／書く対象。parameter、capture、allocation、unknown とアクセスパス |
| memory.returns | 返す参照・値の内部表現の由来 |
| memory.allocations | 割当ての型と位置、returned/stored/host/thrown の経路で外へ出る可能性 |
| facts | 式ごとの node、UTF-8 のバイト位置、操作、到達可能性、i32 の範囲、nullness、配列長、境界・null 検査の根拠 |

関数間で引数・捕捉・戻り値の別名を代入し、格納した参照、closure や値コンテナ、循環参照、loop/recur と再帰を追います。
引数同士が別物であるとは仮定しません。割当て箇所は呼び出しや反復をまたいで併合し、パスが深さ3を超えた部分は unknown へ広げます。
中断や未知の書き込みの後では、可変フィールドに保持していた参照の情報を広げます。

facts の i32_range は符号なしのビット区間です。折り返しやシフト量のマスクを考慮し、区間で表せない場合は全範囲に戻します。
unsigned の `i < array.len(a)`、non-negative を確認した signed 比較、null 検査、変更されないローカルと配列長の対応を保持します。
ループの持ち回り値は初期値のまま扱わず型の上限へ広げ、枝の合流では両方に共通する条件だけを残します。
`bounds_proven=false` は「未確認」を意味します。各事実はその式の正常な継続についてのもので、割当て失敗など別の trap を除外しません。

これらは解析結果です。既存の pure/effect 上限を緩めたり、検査や割当てを削除したりする経路にはまだ使いません。
非 escape だけを根拠に参照同一性や trap を消すこともできません。i64 の区間や明示的なループ不変条件は今後の範囲です。
Rust API は `regions::analyze(&module)` と `flow::analyze(&module, &function)` です。`analysis::analyze` は両方を含めて報告します。

## 実装

```text
ファイル import / include → S 式 → 名前解決・単相化・型検査 → 型付き IR
                                      ├─ 全ソースの解析・契約検査
                                      └─ 到達性による宣言の除去 → WAT → Wasm
```

- `src/ir.rs`: 型、ローカル・関数・ループの ID、閉じた式の列挙型。解析と出力の共通コントラクト。
- `src/sexpr.rs`: 位置情報付きの S 式リーダー。`;;` と入れ子の `(; ;)` コメントに対応。
- `src/source.rs`: 相対 import / include、ファイルの共有と循環検出、診断の元ファイルへの対応。
- `src/modules.rs`: 種類付きの名前表で定義と import 別名を分け、収集・import 解決・宣言の展開の順に処理。
- `src/generics.rs`: 明示した型引数ごとの具体化、再利用、再帰・展開上限と診断。実行用 IR に汎用宣言を残さない。
- `src/bounds.rs`: where の shared/nonunit/callable と effect 上限を、具体的な型・署名・推論 effect で検査。
- `src/type_syntax.rs`: 値型の位置と nominal な関数・heap 型の参照を区別する共通 visitor。
- `src/link.rs`: 検査済みの全 IR から到達する宣言を選び、生成用の ID を振り直す。
- `src/buffers.rs`: bytes/text の private GC storage、UTF-8 検証と値操作。
- `src/frontend.rs`: 型・名前・捕捉・可変性・match の網羅性・tail position の検査。
- `src/typing.rs`: 宣言・推論した型の一覧と、ソース位置からの型の問い合わせ。
- `src/native.rs`: 型付きコア外の標準 WAT の関数シグネチャ。
- `src/runner.rs`: テスト探索、分離した実行、タイムアウト、構造化した結果。
- `src/analysis.rs`: 副作用と呼び出しグラフの解析。文字列の WAT を再解析せず、型付き IR を走査します。
- `src/regions.rs`: 対象別の読み書き、参照の流れ、割当て箇所と escape の解析。
- `src/flow.rs`: 正常な制御フロー上の non-null、i32 区間と配列境界の解析。
- `src/effects.rs`: ソースで観測できる作用、関数外へ出る例外、宣言した effect 上限の検査。
- `src/ownership.rs`: 所有性の伝播、明示 move、分岐・ループ・終了時の所有集合の契約。
- `src/proof.rs`: 型付き IR から BitVec による law の証明義務を生成。
- `src/verified.rs`: Lean の検査を通した同じ IR を Wasm へ出力する経路。
- `proof/Ownership.lean`: 所有権の小さな検査器、宣言的な規則、検査の健全性と二重消費拒否の証明。
- `proof/Delivery.lean`: 配送済み Future/Stream の操作、順序・終端・close のモデルと証明。
- `src/wit.rs`: 公開関数の型から WIT 契約を生成。
- `src/bindings.rs`: event-loop import の固定した型と作用。
- `src/component.rs`: 検査済みの型と core Wasm から Component と manifest を組み立てる。
- `src/component/abi.rs`: portable な型、Canonical ABI のレイアウト・flattening・シグネチャ。
- `src/component/values.rs`: GC 値と ABI 値の変換、bytes/text と複合値のコピー・バッファ解放。
- `src/component/endpoints.rs`: readable handle の転送・read・close。
- `src/component/functions.rs`: core import/export の境界とタスクの cleanup。
- `src/component/core_js.rs`: 検査済みの IR から JS 専用の core Wasm と bindings を生成。
- `src/component/core_js/values.rs`: 必要な具体型の JS codec を生成。
- `src/component-memory.wat`: 変換用バッファの確保・解放。
- `runtime/text-io.mjs` / `node-io.mjs` / `text-cli.mjs`: 値だけを渡す行 I/O と、Node の handle・CLI。
- `runtime/core.mjs` / `build-core.mjs`: JS 専用の Core/JSPI ホストとビルド。状態はインスタンスごとに分離。
- `runtime/`: Component の JSPI 向けの生成、共通の Promise/AsyncIterable ホスト、タスクの要求・終了確認・解放、Node とブラウザの実行テスト。
- `proof/Tasks.lean`: 取消し要求と終了確認の状態モデル、read 終了前のバッファ解放拒否、解放前の終了確認拒否の証明。
- `src/emit.rs`: GC 型、let、for、クロージャ、enum/match、try/catch を通常の WAT に変換。
- `src/lib.rs`: `check(&str)` は型付きコアの IR、`compile(&str)` は標準 WAT と拡張の両方に対応します。Wasm は wasmparser でも検証します。

列挙型はタグとケース別の GC subtype へ変換します。bytes/text は private な packed GC 配列を使い、text は UTF-8 を検証します。
利用者定義の関数・型・alias・クロージャは、`(type.params $T ...)` と `($name T...)` により単相化します。
具体的なクロージャ型を引数にする [map/fold の実例](examples/generic-lists.wlt) と
[具体化時の検査規則](docs/generics.md) を参照してください。
異なるクロージャ型を共通のインターフェースで呼ぶ糖衣、finally、共有 GC ヒープのスレッド間利用の糖衣は、この核の外に置いています。

次の型・effect 設計は Verse の effect family と WASI 0.3.0 の WIT に合わせます。
Option/Result は値、Future/Stream は所有する配送ハンドル、suspends は制御上の作用として分けます。
設計規約・実装範囲は [SPEC.md](SPEC.md)、形状契約は [async-contract.wit](interfaces/async-contract.wit) を参照してください。
値型・effect 契約・所有権・law・配送済み/外部の Future/Stream、WIT 生成、Component ABI adapter、JSPI でのタイマー実行と task cancellation は実装済みです。writer と言語側の producer task は今後の範囲です。
`just wit-check` と WIT のテストには `wasm-tools` が必要で、検証済みバージョンは1.245.1です。


### bytes/text とモジュールを JS から使う

```sh
just build-js tests/component-fixtures/buffers.wlt target/buffers
```

```js
const { api } = await (await import('./target/buffers/index.js')).instantiate();
api.encode('日本語🦀');              // Uint8Array: UTF-8
api.decode(new Uint8Array([255]));   // undefined: option の none
api.text('hello');                  // 'hello'
api.slice(new Uint8Array([1, 2, 3]), 1, 2); // Uint8Array([2, 3])
```

ファイルからは `(import "stdlib/text.wlt" (func $string.u64 $unsigned))` のように、公開する名前を選んで取り込みます。
標準ライブラリの依存と3つのアプリも named import を使います。未使用の定義とホスト import は生成時に除去し、必要な capability だけを manifest に残します。
`include` は互換のテキスト展開として残します。ファイルの読み込みで実行時の I/O capability は増えません。


メモリの管理境界・修正した保持経路・実測は [MEMORY.md](MEMORY.md) を参照してください。`just memory-check` で管理表、GC 到達性、断片化と反復時の保持量を再検査できます。

`stdlib/text.wlt` に連結、u64/s64 の十進数変換と JSON string の生成があります。
検索・分割・検査付き整数解析と text/JSON builder も [標準ライブラリ](stdlib/README.md) にあります。
ログ集計・在庫台帳・workflow は共通の解析関数と JSON object builder を使っています。`just test-stdlib` で境界値と両 JS backend を検査できます。
新しい構文と契約は [SPEC.md](SPEC.md#文字列ライブラリと配列のコピー) を参照してください。
`just fmt-wlt file.wlt` でコメントを保持して整形し、`just check-wlt-format file.wlt` で検査できます。
ログ集計の変更前後は `just compare-log-stats before-directory after-directory` で条件を変えて交互に測定できます。
