# Future / Stream writer と子タスク

writer は既存の所有付き `%resource`、reader は `wlt:core` の `$future<T>` / `$stream<T>` を使う。ホストの `createChannels()` は容量 0–65536 の Stream と、一度だけ確定する Future を提供する。新しい一般的な lifetime 構文は追加しない。

`examples/channels/main.wlt` の `pipeline` は、容量 0 の Stream を言語内で producer と consumer に接続する実行例。`policy.json` も含めて両 JS backend と Chromium で検査する。

```wat
(%use "wlt:core" (type $task))
(%closure $Work (%capture $n i32) (result i32) (%effect pure) (+ $n 1))
(func $main (export "main") (result i32)
  (%effect host reads writes allocates suspends)
  (%task.scope (result i32)
    (%let $a (%task.spawn i32 $Work (%closure.new $Work 20))
      (%let $b (%task.spawn i32 $Work (%closure.new $Work 20))
        (+ (%task.join i32 (%move $a)) (%task.join i32 (%move $b)))))))
```

`%task.spawn T $Closure environment` は引数を持たず `T` を返す宣言済みクロージャを開始し、所有付き `$task<T>` を返す。捕捉は既存の明示宣言を使う。所有捕捉を持つ環境は明示的に move する。結果 `T` は共有可能な型に限定する。結果の GC オブジェクトを JS にコピーせず、同一性を維持する。

`%task.join T handle` は完了を待って結果を取り出す。`%task.drop T handle` は完了を待って結果を捨てる。どちらもハンドルを消費する。`drop` は detach しない。ハンドルのコピー・未消費・ABI への export、任意の数値からの構築は拒否する。

`%task.scope (result T) body` は正常終了と return・br・例外の退出時に子の完了を待つ。公開関数の呼出し全体にも暗黙の親スコープがある。子の失敗は兄弟と親を協調的に取り消す。スコープから例外で退出する場合も親の取消しを要求する。扱いたい失敗は `result` を値として返す。取消しは host 操作・reader・資源の解放と guest stack の unwind を待ってから完了する。

これは一つの JS agent 内の協調的並行処理。Worker の並列実行、CPU ループの強制停止、select、子だけの個別取消しは扱わない。イベントループを止める `atomic.wait` 等を推論 effect が含むクロージャは spawn できない。実行先は Core＋JSPI または Component。Core Wasm 単体を実行する `wlt run` の既定 backend には scheduler がない。

## writer の契約

`runtime/channels.mjs` の `createChannels()` はホスト SDK であり、使用する具体的な payload 型を policy と WLT の import 署名で一致させる。payload は ABI を通せる共有データを値で転送する。GC の参照同一性を保持する子タスクの結果とは境界が異なる。

| 操作 | 契約 |
| --- | --- |
| `stream(capacity)` / `future()` | 所有付き writer の同期 factory。resource `new` として登録 |
| `reader(writer)` | reader を一度だけ取得。resource `reader` 契約が独立した所有 endpoint を生成 |
| `send(writer, value)` | Stream に一要素書く Promise。空きまたは待機 reader がなければ待つ。0=受理、1=終了／取消し |
| `resolve(writer, value)` | Future を一度だけ確定。0=確定、1=既に終了。reader は `future<result<T,i32>>`、未確定 close は err(1) |
| `close(writer)` | 同期・冪等。Stream は受理済みバッファを流して EOF。待機 write は値を保持せず終了 |

一つの reader と一つの pending write に限定する。同時 write / 同時 read は拒否する。reader close はバッファを捨てて待機 writer を起こす。書き込み Promise の disposer は、その書き込みを取り消す。writer の明示 close と `%using` の destructor は既存の資源管理を使う。

`%using` から返せる所有値は、借用を含められない Future / Stream / Task に限って拡張した。例えば `%using` 内で consumer を spawn し、その Task を返せば、writer を閉じてから外側で join できる。borrow・その他の所有値の返却禁止は維持する。

## 実装と検査の範囲

GC slot と scheduler の private ABI は数値 ID だけを渡す。Component の固定版 jco は、親タスク内の別 thread として子を実行する補正を `runtime/jco-children.mjs` に限定する。元の生成パターンが変われば build を拒否する。スレッドごとの import 対応、終了時の登録解除、親 Wasm stack の unwind 完了を検査する。

所有権は既存の Rust / Lean 契約を再利用する。scheduler・JSPI・glue・finally の意味保存や liveness の機械証明はまだない。`just test-runtime-cli` と `just test-e2e` は、並行開始の barrier、参照同一性、EOF、取消し、兄弟失敗、ABI の未解放数と GC slot 数 0 を確認する。

## 期限とホスト側スコープ

非同期 export を含む JS 成果物は `withTaskScope` も export する。

```js
import {instantiate, withTaskScope} from './app/index.js';
const {start} = await instantiate();

const call = start('work', [], {timeoutMs: 1000});
await call.result; // 取消し・子タスク・host disposer の終了まで待つ

const result = await withTaskScope(start, async scope => {
  const first = scope.start('work', []);
  scope.start('anotherWork', []);
  return first.result;
}, {timeoutMs: 1000});
```

`timeoutMs` は0–2147483647の整数。start に渡した時点から queued の時間も数える。
0 は guest に入らず取り消す。期限切れは `TaskTimeoutError` で、`TaskCancelledError` の派生型。
guest の完了が先なら、遅れた期限は結果を変えない。timer と AbortSignal listener は終端で外す。

公開呼出し一回が、既存の暗黙の親スコープ。期限切れはその呼出しと WLT の子を取り消し、
子の unwind・資源の解放が終わってから result を reject する。別の公開呼出しは取り消さない。
同期 api の戻り値と、非同期の disposer を含む終了通知は別で、解放完了を待つ場合は start の result を使う。

withTaskScope は callback と、その scope.start で開始した公開呼出しを束ねる。
正常終了でも全呼出しを待ち、callback／呼出しの失敗・外部 AbortSignal・期限で所属する呼出しを取り消す。
scope.cancel(reason) も解放を待つ。終了後の start は拒否し、終了済み scope は instance や過去の結果を保持しない。
同一 instance の公開呼出しは従来どおり直列化される。並行に進める処理は WLT の子タスクを使う。

これは公開呼出しとホスト側のグループを単位とする取消し。
`%task.cancel T handle` はハンドルを消費し、その子と孫の取消し・unwind・host disposal を待つ。親と兄弟は継続する。`drop` は完了待ち、`cancel` は取消し要求という違いがある。

`%task.join-timeout T milliseconds handle` はハンドルを消費して `option<T>` を返す。完了すれば `some`、時間切れなら子の取消しと解放が完了してから `none`。待ち時間は非負の i32、0 も指定できる。既に完了した子の結果を優先する。通常の trap や cleanup failure は成功扱いにせず、親の失敗になる。

取消しは協調的で、CPU 処理を強制停止しない。ホスト操作は disposer と完了通知の契約を守る必要がある。スコープごとに reader、所有 endpoint、resource、ABI allocation、保留 disposal を追跡する。子へ move した捕捉の所有先も移す。ABI 戻り値は post-return まで生存させ、取消しで破棄する子のバッファだけを回収する。

`proof/Tasks.lean` は取消し対象外の状態を保存する契約と、解放前に acknowledgment できない契約を検査する。JS 実装全体の機械検証ではない。任意スコープの取消しトークンと select は後続。
ABI を越えてホストへ返した Future/Stream はホストへ所有が移り、その寿命まで deadline が延びるわけではない。
callback と host 操作は協調的に終了する必要があり、CPU ループや応答しない provider を timer で強制停止できない。

## 所有する timer future

`wlt:host/event-loop@0.1.0.after(i32)` は `future<unit>` を返す固定の clock capability。
既存の Future と同じ所有権・await・close を使う。

```wat
(%use "wlt:core" (type $future) (type $unit))
(import "wlt:host/event-loop@0.1.0" "after"
  (func $after (param $ms i32) (result (%inst $future $unit))))
(func $pause (export "pause") (param $ms i32) (%effect host suspends)
  (%future.await $unit (call $after $ms)))
```

close／取消しでは timer を解放する。型・effect・capability は既存の trusted binding と照合する。
