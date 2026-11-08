# 簡潔データ構造を安全に並列照会するための契約

## 現行WLTで何が検査・証明済みか

### Takeaway
共有線形memory＋ホストWorker＋atomicsは実装済みだが、任意の共有領域の所有権・freeze・read-only region・書込みshardの証明は未実装。現行 `%race-free` は固定atomic cell限定であり、普通の `i32.load/v128.load` によるreadonly index走査も証明モードでは拒否する。

### Cited Findings
- `src/threads.rs` は線形メモリをモジュールcapabilityとし、owned GC値・transferable refとは扱わない。67 threads命令の型/effectと、同じMemoryをHost JS/Workerで使う実行テストがある。 — [memory contract](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/threads.rs:1)、[公開仕様](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/threads.md:31)、[実Worker検査](/Users/mz/ghq/github.com/mizchi/wlt-lang/runtime/threads.test.mjs:13)
- 通常モードの型付きWLTはscalar/SIMD線形load/storeを扱い、`v128.load/store/lane`も受理する。Wasm validatorが型・即値・alignmentを検査するが、region ownershipを与えるものではない。 — [許可命令](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/wasm.rs:5)、[validator bridge](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/wat-validator.md:15)
- `%race-free` は固定shared memory最大1、initial=maximum、raw memory exportなし、static cell最大256、幅i32/i64・非重複・自然alignmentを要求する。全関数を検査し、host function importも拒否。 — [実装](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/race.rs:131)
- 特に `ExprKind::Wasm` を全面拒否するため、読み取りしか行わない `v128.load` でも現行証明書に入らない。TaskSpawn/Join/Scopeもここで拒否する。 — [決定的な拒否箇所](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/race.rs:209)、[raw loadの拒否テスト](/Users/mz/ghq/github.com/mizchi/wlt-lang/tests/race_free.rs:54)
- Leanのカーネルは全アクセスを宣言セルと同一offset・幅のatomicに限定し、任意数のagent・無界の有限prefixについてrace_free、uniform_cells、in_boundsを証明する。現行DataRace述語はhappens-beforeを推論せず、重なる非atomic書込みの組を保守的に禁止する。 — [traceモデル](/Users/mz/ghq/github.com/mizchi/wlt-lang/proof/AtomicCells.lean:63)、[定理](/Users/mz/ghq/github.com/mizchi/wlt-lang/proof/AtomicCells.lean:129)
- 証明はHost JS・他Workerもセル契約を守ることが前提。`atomicHostContract` の指定はホストコードの検証ではない。load→add→storeで更新喪失する反例を保持し、linearizability、deadlock-free、Owned<R> split/moveは保証しない。 — [ホスト前提](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/atomic-cells.md:43)、[保証の限界](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/atomic-cells.md:69)
- `%where (shared T)` は同一agentでコピー可能という意味で、Rust Send/Syncではない。通常のGC ref/box/future/resourceをworkerへ渡す所有権規則はなく、shared GC/shared-everythingは現行WLTの対象外。 — [型の意味](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/threads.md:79)
- `%task.spawn` は同じJS agent内の協調的並行処理でありWorker並列ではない。同期CPU loopの強制停止やatomic.waitの中断を既存task cancellationが保証するわけではない。 — [taskの範囲](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/scoped-tasks.md:18)、[atomic.waitと取消し](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/threads.md:85)
- この実装は共有memoryのimport/exportをComponent/WIT境界では `E_COMPONENT_MEMORY` で拒否する。外部共有memoryをWorker間で使う初版はCore/JSの配布が対象になる。 — [backend境界](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/threads.md:87)

### Inferences
- 推奨する表現は、**index本体を共有線形memory、各Workerのwrapper/メタデータ/query scratchをWorker-local GC**。線形memory内にGC pointerを格納せず、offset/length/bit-length/version等の整数だけを置く。SIMDもその共有領域から直接ロードできる。
- 現行機能だけで「動作する共有index」は書けるが、「型とLeanによりreadonly sharingが検査済み」とはまだ言えない。静的安全性を売りにするなら読み取りregionの契約拡張が必要。
- 共有化はWLT固有の優位ではない。JSも同じSABとWorkerを使える。比較は同じWorker数・同じimmutable表現・同じbatch granularityを揃え、WLTのpopcnt/SIMD/bit操作による追加利益を見る必要がある。

### Gaps
- freeze後の任意raw write禁止、region内のpointer provenance、Worker間の所有トークン移動、join後の領域再利用を静的に検証する機能はない。
- 任意のJS hostがSharedArrayBufferを書き換えないことをWLTの型だけで保証する手段はない。JS objectのfreezeをしても共有bufferのbyteをread-onlyにするわけではない。

## 最小の安全な設計はどのようなものか

### Takeaway
**単一builder → validate → freeze/publish → read-only parallel queries → 全readerのjoin → release** が最も小さく、安全性を説明しやすい。並列書込みbuilderや共有出力バッファは初版から入れず、まずWorker私有の結果をmessageで返す。

### Cited Findings
- Wasm threadsはshared linear memoryとatomicsを定義し、スレッドのcreate/joinはembedderに任せる。atomic操作はseq-cstであり、WLT独自のacquire/release構文を先に作る必要はない。 — [Wasm threads提案](https://github.com/WebAssembly/threads/blob/main/proposals/threads/Overview.md#threading-proposal-for-webassembly)、[Atomic memory accesses](https://github.com/WebAssembly/threads/blob/main/proposals/threads/Overview.md#atomic-memory-accesses)
- ECMAScript memory modelは、同じmemory rangeのseq-cst writeを読んだseq-cst readをsynchronizes-withに含める。agent orderとsynchronizes-withからhappens-beforeを構成する。hostのpostMessageも追加同期を与える機構の例として挙げる。 — [ECMAScript synchronizes-with](https://tc39.es/ecma262/multipage/memory-model.html#sec-synchronizes-with)、[happens-before](https://tc39.es/ecma262/multipage/memory-model.html#sec-happens-before)
- SABのstructured serializationは同じデータブロックを別agent側のオブジェクトとして復元する。ブラウザでは同じagent clusterとcross-origin-isolated capabilityの制約がある。 — [HTML structured serialization](https://html.spec.whatwg.org/multipage/structured-data.html#structuredserializeinternal)
- Wasmのimportされたshared memoryへactive data segmentを持つmoduleを各Workerでinstantiateすると、その度にdata segmentがコピーされる。仕様は一回だけ初期化するmoduleとmain moduleを分ける例を示す。 — [Wasm initializing memory only once](https://github.com/WebAssembly/threads/blob/main/proposals/threads/Overview.md#initializing-memory-only-once)
- Web Workerは起動費用と1instanceあたりのmemory費用が高く、長寿命を想定する旨がHTML仕様に記される。 — [HTML worker scope](https://html.spec.whatwg.org/multipage/workers.html#scope-2)

### Inferences
次は**未実装の設計案**であり、現在の構文として提示しない。

1. **Builder段階**: 一つのownerだけがpayload領域を書ける。動的GC構造や一時bufferを使ってよいが、共有形式へ並べ終えてからvalidateする。最初はbuilderをsingle-threadに固定し、並列buildの分割所有権を要求しない。
2. **Validate段階**: memory32範囲、offset+lengthのoverflow、bit-length、word padding、rank directory、index種類/version等を検査する。race-free証明と、rank/select等のindex内容の正しさは別の契約。
3. **Freeze段階**: builderの書込み権を消費し、読取り用descriptorだけを作る。payloadは以降変更しない。後述の最小証明書案では初期化をhost前提に置けるが、それは静的なfreeze所有権の証明とは呼ばない。
4. **Publish段階**: payloadとdescriptorを書き終えた後、専用SC ready/version cellへstoreする。各readerは同じSC cellをloadし、期待versionを観測してからpayloadを読む。`notify`だけをpublish barrierの代用にせず、ready値を再確認する。初回はfreeze完了後にMemory/descriptorをpostMessageで配る構成も使いやすい。
5. **Query段階**: immutable payloadを複数Workerが非atomic scalar/SIMD loadで読む。query scratch、temporary GC objects、result builderはWorker私有。worker moduleには共有payloadへ書くdata segment/init/store経路を持たせない。
6. **Join/release段階**: 新規queryを止め、稼働中queryの終了を確認するまでsnapshot領域を再利用しない。初版はsnapshot全体をjob scopeが保持し、distributed refcountやhazard pointerは導入しない。

並列の切り方は最初に**独立したqueryをWorkerへbatch配送**。一つの巨大queryを分割する場合もread-only indexのword-aligned連続範囲を割り当て、Worker私有でcount/top-kを計算してcoordinatorでmergeする。毎rank/select呼出しをmessageにすると通信費用が支配する。

結果は初版では `postMessage({generation, queryId, count/topK})` で返す。大きな結果ならWorker私有のArrayBufferをtransferする。**共有result slotへの書込みそのものを不要にする**ため、readonly regionの安全性から先に検証できる。Worker poolを使い、jobごとにWorkerを作らない。

### Gaps
- 現行型/ownershipはfreezeの状態遷移やregion tokenを持たない。`%box i32`にpointerを包むだけではpointerが指す範囲の一意所有を証明できない。
- 現在の`%race-free`カーネルはhappens-beforeやready値の観測をモデル化していない。SC命令を使う設計が妥当でも、既存カーネルがpublish protocolまで証明したとは言えない。

## 必要な最小拡張と証明境界

### Takeaway
最初の拡張は、汎用borrow checkerやロック検証ではなく、**公開後不変のread-only byte region＋既存atomic control cells**に限定するのが現実的。writable shard、世代をまたぐ再利用、lock-free更新は別段階にする。

### Cited Findings
- 現行カーネルは `SourceEvent` がatomic fixed cellに適合することだけを検査し、任意順の全アクセスを許す。共有前の初期化は任意初期状態としてモデル外に置く。従ってphaseを区別する拡張か、post-publication traceだけを検証する追加前提が必要。 — [Source/Host events](/Users/mz/ghq/github.com/mizchi/wlt-lang/proof/AtomicCells.lean:66)、[共有前初期化](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/atomic-cells.md:51)
- `read/write` effectは命令分類であり、未知線形メモリのaliasを排除しない。整数pointerはownedとして扱わない。 — [raw memory解析](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/wat-validator.md:19)、[pointer所有の未対応](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/threads.md:83)

### Inferences
**段階A: 最小のimmutable-region証明書（新規版）**

- 初期memoryは固定サイズにし、readonly payload rangesとSC control cellsを宣言する。payloadとcontrolは非重複。readonly同士のoverlapはread/readなので許してもよい。
- index queryには「あるreadonly region内」というprovenanceを持つchecked loadを使う。dynamic offsetでは `width <= length && offset <= length-width` を検査し、overflowなくregion内を確認する。scalarとv128の16byte読取りを明示分類する。単なるWasm全memoryのbounds検査だけではregion境界を保証できない。
- post-publication sourceではそのpayloadへのstore/fill/copy destinationを禁止する。読み取りcopy sourceはreadonly regionとして検査。atomic cellへのアクセスは現行の厳格なwidth/offset規約を維持する。未知raw accessをfallbackで受理しない。
- カーネルの不変条件を「readonly rangesではreadのみ、競合する書込みはuniform atomic cellsのみ」とする。read/readはraceを作らないため、固定cellより小さな追加定理で始められる。ただしpublicationとreleaseは別のphase/lifetime契約。
- 初版では「初期化が全readerの開始より前、公開後hostもpayloadを書き換えない」をhost契約に置ける。manifestへreadonly ranges・version・alignment・control cell layoutを出す。これは**ホスト契約付きのread-only sharing検証**で、任意JSの静的検証ではない。

**段階B: freezeを検査済みにする**

- mutable builderのaffine capabilityからreadonly capabilityへの変換を導入する。freezeはbuilderの権限を消費し、書込み別名やraw i32への脱出を禁止する。WLT source内のstoreもcapability経由に限定する必要があり、既存box/newtypeだけのlibrary実装を静的一意所有と呼ばない。
- readonly capabilityのコピーは可能、変更・再初期化は不可。実bufferを握るJS/未検査Wasmは引き続き信頼境界。public wrapperでMemoryを公開しないことは誤用を減らすが、任意host書込みの不可能性を証明しない。
- readerを持ったまま同じ領域をbuilderに戻さない。最初はsnapshotを捨てて新bufferを作る方式とし、世代のreuseやlock-free memory reclamationを不要にする。

**段階C: 並列builderや共有出力が必要になった場合だけ**

- unique writable regionを非重複shardへsplitし、各Workerへmoveする。各writeはshard内、shard間は非重複、戻りtokenのjoin前に親からアクセス禁止。これを静的に証明するにはdynamic region bounds/provenanceとmove/joinのルールが新たに必要。
- 共有result slotはWorkerごとの固定非重複範囲、一つのwriter、generation別の予約状態にする。payload書込み後にSC done store、coordinatorは期待generationのdone load後だけ読む。前jobの全writer終了前にslotを初期化/再利用しない。atomic doneだけがあってもpayloadの排他・寿命は自動的に証明されない。
- false sharingやメモリ帯域の飽和はrace-freeとは別の性能問題。padding/word分割/batchサイズは計測で決める。

### Gaps
- 段階A/B/Cはいずれも現行実装ではない。新しい証明書schema、Rust extractor、Lean model、codegen、host wrapper、拒否witness、Node/ChromiumのWorker検査を揃える必要がある。
- readonly payloadだけならrace-freeは比較的単純だが、publish-readyの正しい観測、release後の再利用、分散lifetime、linearizabilityまでを一つの固定cell証明に押し込めるべきではない。

## 取消し・終了・resultの寿命をどうするか

### Takeaway
取消し要求とWorkerの停止完了を分け、**cancelしてもjoinが済むまで共有snapshot/result slotを保持する**。既存のJSPI task取消しがWorkerのCPU loopを止めるとは扱わない。

### Cited Findings
- 現行WLT task modelも取消し要求と終了確認を分け、owner/buffer/disposal等が残る間はcleanでないとしている。ただし同agent協調taskのモデルで、Worker protocolの証明ではない。 — [既存Tasksモデル](/Users/mz/ghq/github.com/mizchi/wlt-lang/proof/Tasks.lean:3)、[task実行範囲](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/scoped-tasks.md:24)
- Node v24系の `worker.terminate()` は非同期で、exit event時に解決するPromiseを返す。 — [Node v24 worker.terminate](https://nodejs.org/docs/latest-v24.x/api/worker_threads.html#workerterminate)
- HTMLのWorker terminate処理はclosing設定・queued task廃棄・実行scriptのabortを行う。ブラウザのterminate APIはNodeのPromise joinと同一ではなく、通常のguest cleanup/finally完了を期待する設計にしない。 — [HTML terminate a worker](https://html.spec.whatwg.org/multipage/workers.html#terminate-a-worker)

### Inferences
- jobごとにSC cancel cellとgenerationを持ち、カーネルは大きなchunkの境界でcancelをloadする。poll頻度は取消しレイテンシと性能のトレードオフで測る。同期waitを使うworkerにはfinite timeoutかnotifyを用意する。
- coordinatorはキャンセル要求後もjob所有snapshot・scratchの管理情報を保持し、全workerのdone/cancelled acknowledgmentを待つ。エラーWorkerも終了確認へ合流させる。UI側Promiseを先にrejectする場合も、内部cleanup/joinのPromiseは必ず保持する。
- Nodeの強制終了では `await worker.terminate()` を待つ。ブラウザは協調ackを標準経路とし、強制terminateではguestの解放手続きに依存しない。未確認のslotを次jobへ再利用しない。readonly snapshotを丸ごと世代ごとに管理すれば、ハングした旧readerと新writerの衝突を避けやすい。
- generation付きresultは、旧jobの遅いmessageを次jobの結果へ混入させない。ただしgenerationチェックだけで旧Workerのメモリアクセスが止まったとは言えず、再利用にはjoinが必要。
- この安全性はデータ競合排除と寿命の規約である。deadlock-free、lock-free進行、公平性、linearizabilityの証明は別。parallel count/rankの結果正当性も別のlaw/oracleで検査する。

### Gaps
- 現行WLTのWorker pool・shared snapshot job scope・cancel polling・join/release protocolは未実装。既存 `%task.scope` をそのままWorker scopeと呼ばない。
- 新しい性能基準では1/2/4/8 Workerの同じquery、poolを温めた状態と起動込み、構築＋freeze時間、snapshotを含む総memory、cancel-to-joinのp95、重いwriterを含まないread-onlyの帯域限界を測る必要がある。速度向上は未測定の仮説。
