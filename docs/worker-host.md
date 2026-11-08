# 汎用の非同期Workerホスト

[WorkerHost](../runtime/workers/host.mjs) はNodeのworker_threadsとブラウザのmodule Workerを同じAPIで扱う。succinct索引、Wasm、特定のデータ形式には依存しない。起動・キュー・配送・取消し・join・終了を担当し、アプリケーション固有の処理はdriverへ分ける。

```js
import { WorkerHost } from './runtime/workers/host.mjs';

await using host = await WorkerHost.create({
  workers: 4,
  driver: new URL('./my-driver.mjs', import.meta.url),
  init: { bias: 2 },
});

const result = await host.run({ numbers: [20, 20] });
const results = await host.map([
  { numbers: [10, 20] },
  { numbers: [40] },
]);
const task = host.start({ numbers: [1, 2] });
await task.result;
await task.cancel(); // 完了後なら何もしない
```

driverは通常のES moduleとして書く。関数・クロージャを文字列化してWorkerへ送る仕組みは持たない。

```js
// my-driver.mjs
export async function create({ bias }) {
  return {
    async run({ numbers }, { signal, workerId, isCancelled }) {
      if (signal.aborted || isCancelled()) throw signal.reason;
      return { value: numbers.reduce((sum, n) => sum + n, bias) };
    },
    async dispose() {
      // このWorkerが所有する外部資源の終了を待つ。
    },
  };
}
```

`create`と`run`は同期関数でもよい。`create`の結果は`run`を必須とし、`dispose`は任意。`run`は`{value, transfer?}`を返す。TypedArrayをコピーせず返す場合は、例えば`{value: output, transfer: [output.buffer]}`を使う。

## APIと配送の規約

| API | 規約 |
|---|---|
| `WorkerHost.create({workers, driver, init, cancellation?})` | 1〜32 Worker。全driverの初期化を待つ。initは一度snapshotし、同じclone可能な値を各Workerへ渡す |
| `run(payload, {signal?, transfer?})` | 空いているWorkerに1ジョブを割り当て、結果のPromiseを返す。同時に複数回呼べる |
| `map(payloads, options)` | Worker数を超えるジョブもキューに置く。結果は投入順。待機キューは最大4096ジョブ |
| `start(payload, options)` | `{result, cancel}`のハンドル。`cancel(reason)`は実行中のジョブの終了確認まで待つ |
| `join()` | 呼出し時点で投入済みのジョブをすべて待つ。エラー・取消しも終了として待ち合わせる |
| `close()` / `Symbol.asyncDispose` | 新規投入を止め、待機ジョブを取り消し、実行中ジョブをjoinし、各driverのdisposeとWorker終了を待つ |

デフォルトのジョブpayloadは配送時にstructured cloneする。キュー内で待っているpayloadを変更しない。`transfer: payload => [payload.bytes.buffer]`を指定した場合、そのbufferは配送時に移動し、送信元から使えなくなる。共有メモリは内容をコピーせず共有する。Wasm GC参照、Promise、関数は配送する値として扱わない。

`start().result`には`Symbol.asyncDispose`も付ける。既存のWLT Future host adapterへこのPromiseを渡す場合、Futureのcloseをジョブの取消しとjoinへ接続できる。FutureそのものをWorker間で送る機能ではない。

driverはtrustedなアプリケーションコード。ホストが任意のJSを検証・隔離するわけではなく、任意の共有データを自動でrace-freeと認定することもない。

## 取消しと共有メモリの寿命

非同期driverにはWorker内の`AbortSignal`を渡す。取消しメッセージで中断を要求し、`run`が終了を返すまで呼出し元のPromiseを確定しない。一つのジョブ・mapグループの取消しは、別のグループを取り消さない。最初の取消し・エラーを理由として保持し、`0`・`false`・空文字・nullの理由も失わない。

CPU処理はWorkerのイベントループを止めるため、メッセージだけでは実行中に取消しを観測できない。任意で次を指定する。

```js
cancellation: { memory: sharedMemory, byteOffset: 0 }
```

この自然アライメントのi32セルをホスト専用の取消しbitmaskにする。Worker IDのbitだけをAtomicsで設定・解除する。JSのCPUループは`context.isCancelled()`、Wasm側は同じbitを定期的に読む。セルは他のホスト／プロトコルと共用しない。汎用ホストは関数の任意位置を強制的に巻き戻さない。

ホストはinitのsnapshotと制御セルを、すべての実行が終了しdriverのdisposeとWorker終了が確認できるまで保持する。取消し要求、世代番号の変更、Promiseの早期rejectだけで共有領域を再利用しない。終了処理に失敗した場合はcloseをrejectし、leaseを保持して再試行を許す。

ブラウザでSharedArrayBuffer／共有Wasm Memoryを使うdriverにはcross-origin isolationが必要。共有メモリを使わない通常のWorkerジョブにも、その条件を一律には要求しない。

## WLTとの接続

[別の非同期WLTプログラム](../tests/component-fixtures/worker-job.wlt)と[driver例](../runtime/testing/worker-wlt-driver.mjs)を用意した。WorkerごとにCore＋JSPIまたはComponentをinstantiateし、Worker内でFutureをawaitする。結果のi64だけをBigIntとして戻す。取消しを既存runtimeの`start(..., {signal})`へ渡し、後片付けの完了まで待つ。

`%task.spawn`は同じエージェント内の協調的並行処理のまま。GC環境を別Workerへ暗黙に送る操作には変えない。Workerへ渡すデータは明示したclone／transfer／共有メモリの契約に従う。

[succinct driver](../runtime/succinct/driver.mjs)は、この汎用ホストの一つの利用例。索引layout、Wasmのreserve/batch関数、出力の型を知るのはdriverとQueryPool adapterだけで、汎用ホストにはそれらを持ち込まない。

`just test-workers`で、非同期ジョブ、CPU処理の個別取消し、transfer、取消し後の全reader待合せ、終了失敗時の再試行、両WLTバックエンド、Chromiumを検査する。ホストのJS全体を形式証明したものではない。共有索引のデータ競合に関する検証範囲は[読み取り専用領域](readonly-regions.md)を参照。
