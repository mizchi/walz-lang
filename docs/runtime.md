# Future / Stream と JavaScript 実行

```wat
(%let $future (%future.ready i64 (i64.const 42))
  (%future.await i64 (%move $future)))
```

`(%inst $future T)` と `(%inst $stream T)` は opaque な所有値です。ready/empty/cons で構築し、await/next/close だけで扱います。
next は `(%inst $pair (%inst $stream T) (%inst $option T))` を返すので、次回は返された Stream を使います。終端の none でも端点が返り、最後に明示 close が必要です。
payload は共有できるデータに限定し、unit、Result、GC 参照も扱えます。Future/Stream 自体や own 値を payload にすることは初版では拒否します。
await/next は suspends を持ち、呼び出し元へ伝播します。WIT の future/stream 型と読み取り関数の async にも対応します。

[ready-delivery.wlt](../examples/ready-delivery.wlt) は stream の40と2を合計し、別の completion Future の Result も確認して42を返します。
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

[event-loop.wlt](../tests/component-fixtures/event-loop.wlt) は、GC 参照を待機中も保持する main、非同期 Stream の総和、completion Result の分岐、pending Future の close を実装しています。
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

生成する JS には、固定した jco バージョンの unit/option.none の配送と、失敗した import の Promise が確定しない問題への局所的な補正を適用します。内容は [jco-compat.mjs](../runtime/jco-compat.mjs) に集約し、実行テストで両方向の配送と失敗を確認します。
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
API の制約・内部 ABI は [SPEC.md](../SPEC.md#corejspi-の-js-専用-abi)、サイズ・メモリの比較は [MEMORY.md](../MEMORY.md) を参照してください。

`just profile before` / `just profile after` で行フィルタの guest profile と実行メトリクスを保存し、
`just profile-compare before after` で保存した成果物を交互に再測定できます。
ABI 割当てを約19%減らした変更、UTF-8 検証のチューニングと測定条件は [PROFILE.md](../PROFILE.md) に記録しています。

少し大きい実用例として、[ログ集計 CLI](../examples/apps/log-stats/README.md) を追加しました。
静的モジュールと文字列ライブラリで TSV の解析、GC 上の集計、整数の文字列化、JSON の1行出力を実装しています。
Result / Stream のマクロで所有権を保つ反復を記述し、集計状態は opaque な型として公開します。
`just build-log-stats` の後、`just log-stats input.tsv` で実行、`just test-log-stats` で検査できます。
`just profile-log-stats` は、共有する計算部分の guest profile と、実際の Core＋JSPI 経路の実行時間を保存します。

改善後の機能を別の処理で確認する [在庫台帳 CLI](../examples/apps/stock-ledger/README.md) もあります。
`just build-stock-ledger` → `just stock-ledger` で、Unicode の品目名と数量から JSON Lines を出力します。
手書き版とマクロ版を同じ操作列で照合し、`just audit-stock-ledger` で記述量・生成 IR・実行時間を比較できます。

### UTF-8 行フィルタと I/O capability

```sh
just build-text
printf '日本語🦀\n\nlast\n' | just text-filter
just text-file input.txt output.txt
just test-io
```

[行フィルタ](../examples/io/text-filter.wlt) は、空行を除いて入力を出力し、出力した行数を `result<i32,i32>` で返します。
入力は LF / CRLF 区切りで、末尾の改行なし、チャンク途中で分割された UTF-8 に対応します。出力は各行に LF を付けます。
CLI の stdout は処理したデータだけで、終了コードは成功0、I/O の失敗1、SIGINT による取消し130です。同じファイルを入力と出力へ指定することは拒否します。

固定の契約は [text-io.wit](../interfaces/text-io.wit) の `wlt:host/text-io@0.1.0` です。

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

Wasm 例外と trap の違いや、標準 WAT と拡張を混在できる範囲は [構文と解析の境界](syntax.md#標準-wat-との混在と解析) を参照してください。
`%try` は `try_table` へ展開し、trap は捕捉しません。[exceptions.wlt](../examples/exceptions.wlt) に実行例があります。

## bytes/text とモジュールを JS から使う

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

標準関数は `(%import "wlt:std" (func $string.u64 $unsigned))` のように、公開する名前を選んで取り込みます。
標準ライブラリの依存と3つのアプリも named import を使います。未使用の定義とホスト import は生成時に除去し、必要な capability だけを manifest に残します。
`%include` は互換のテキスト展開として残します。ファイルの読み込みで実行時の I/O capability は増えません。


メモリの管理境界・修正した保持経路・実測は [MEMORY.md](../MEMORY.md) を参照してください。`just memory-check` で管理表、GC 到達性、断片化と反復時の保持量を再検査できます。

`stdlib/text.wlt` に連結、u64/s64 の十進数変換と JSON string の生成があります。
検索・分割・検査付き整数解析と text/JSON builder も [標準ライブラリ](../stdlib/README.md) にあります。
ログ集計・在庫台帳・workflow は共通の解析関数と JSON object builder を使っています。`just test-stdlib` で境界値と両 JS backend を検査できます。
新しい構文と契約は [SPEC.md](../SPEC.md#文字列ライブラリと配列のコピー) を参照してください。
`just fmt-wlt file.wlt` でコメントを保持して整形し、`just check-wlt-format file.wlt` で検査できます。
ログ集計の変更前後は `just compare-log-stats before-directory after-directory` で条件を変えて交互に測定できます。
