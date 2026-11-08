# 共有線形メモリと atomics

[Wasm threads の仕様](https://github.com/WebAssembly/threads/blob/main/proposals/threads/Overview.md)に沿い、スレッドの生成・join はホストが担当する。WLT は共有メモリへのアクセスを型付き IR で検査し、WAT の命令を直接出力する。スレッド用の独自ランタイムや GC 間のメッセージ配送は追加しない。

## ホストのメモリを使う

`counter.wlt`:

```wat
(module
  (import "env" "memory" (memory $shared 1 2 shared))
  (func $add (export "add") (param $address i32) (param $n i32) (result i32)
    (%effect reads writes synchronizes)
    (i32.atomic.rmw.add $shared $address $n)))
```

```sh
node runtime/build-core.mjs counter.wlt target/counter
```

```js
const { instantiate } = await import('./target/counter/index.js');
const memory = new WebAssembly.Memory({ initial: 1, maximum: 2, shared: true });
const { api } = await instantiate({ imports: { env: { memory } } });
const words = new Int32Array(memory.buffer);
api.add(0, 1);                 // 変更前の値 0
Atomics.add(words, 0, 2);       // 同じ領域の変更前の値 1
api.add(0, 0);                 // 3
```

同じ `memory` を Worker に structured clone で渡し、各 Worker でも import して使う。メモリの内容はコピーしない。Core＋JSPI の ABI 一時領域は別の非共有メモリであり、ホストのメモリを allocator や cleanup の対象にしない。実行可能な使用例は [Node の検査](../runtime/threads.test.mjs) と [ブラウザ Worker の検査](../runtime/e2e/threads.spec.mjs) にある。

型付きの宣言では `$name` を付け、memory32 のページ数を使う。1ページは64KiB、上限は65536ページ。`shared` には最大ページ数が必須で、初期値は最大値以下にする。ホストからの import は Wasm エンジンが共有属性・最小／最大ページ数を検証する。`shared` のないメモリにも atomics を使えるが、wait は実行時に trap する。

`(memory $private 1 2 shared)` のような定義、inline の `(export "memory")`、`(export "memory" (memory $shared))` にも対応する。ファイルライブラリでは `(%export (memory $mem))` と `(%use "./counter.wlt" (memory $mem $shared))` で選択・別名を指定できる。非公開メモリは外から参照できず、未使用のメモリ import は DCE で落とす。

## 命令と型

元の threads 提案の67命令を扱う。命令名には `%` を付けない。

| 命令群 | アドレス以外の入力 | 結果 |
| --- | --- | --- |
| `i32/i64.atomic.load`、`load8_u` / `load16_u` / `i64.atomic.load32_u` | なし | i32 / i64 |
| `i32/i64.atomic.store`、`store8` / `store16` / `i64.atomic.store32` | i32 / i64 | unit |
| `i32/i64.atomic.rmw[8/16/32].{add,sub,and,or,xor,xchg}[_u]` | i32 / i64 | 変更前の値 |
| `i32/i64.atomic.rmw[8/16/32].cmpxchg[_u]` | 期待値・置換値 | 変更前の値 |
| `memory.atomic.wait32` | i32 の期待値、i64 の timeout | i32 |
| `memory.atomic.wait64` | i64 の期待値、i64 の timeout | i32 |
| `memory.atomic.notify` | i32 の通知数 | i32 |
| `atomic.fence` | なし。メモリ指定もなし | unit |

アドレスはバイト単位の i32。狭い幅の load / RMW はゼロ拡張し、store は下位ビットを格納する。i32 に32bitの狭幅形、i64 に64bitの狭幅形はなく、通常形を使う。RMW の狭幅形は `_u` を付ける。

`memory.size $mem` はページ数を返す。`memory.grow $mem pages` は変更前のページ数、失敗時は -1 を返す。ホスト側で grow 後の長さを使うときは `memory.buffer` から view を作り直す。

```wat
(i32.atomic.load $shared offset=8 align=4 (i32.const 0))
(i64.atomic.rmw.cmpxchg $shared 16 (i64.const 0) (i64.const 1))
(memory.atomic.wait32 $shared 0 1 (i64.const 1000000))
(memory.atomic.notify $shared 0 1)
(atomic.fence)
```

offset は unsigned memory32 の即値。align はアクセス幅と同じバイト数だけを許す。実際のアドレスの不整列や範囲外は Wasm の trap に従う。すべて sequentially consistent で、memory order を指定する独自構文はない。

ルートではメモリ名／番号を省くとメモリ0を使う。WLT のリテラル省略は入力数で区別し、`(i32.atomic.load 0)` はアドレス0、`(i32.atomic.load $shared 0)` はメモリ名とアドレスを表す。ファイルモジュール内では別ファイルの追加で参照先が変わらないよう、必ずメモリ名を指定する。anonymous memory、memory64、非 atomic な load/store、共有 GC / shared-everything threads は型付きの追加範囲に含めず、検証済みの標準 WAT の経路を維持する。

## effect と所有権の契約

| 操作 | semantic effect |
| --- | --- |
| atomic load | `reads synchronizes` |
| atomic store | `writes synchronizes` |
| atomic RMW / cmpxchg | `reads writes synchronizes` |
| wait32 / wait64 | `reads synchronizes blocks` |
| notify / fence | `synchronizes` |
| shared memory の size / grow | `reads synchronizes` / `reads writes synchronizes` |

`synchronizes` は同期操作の順序を観測できること、`blocks` は現在のエージェントを同期的に止め得ることを表す。呼出し先からも推論し、`pure` の上限には入らない。`blocks` は JSPI の `suspends` を追加しない。解析にも `synchronizes` / `may_block` を表示する。

メモリはモジュールが参照する共有 capability で、owned な値ではない。既存の `%where (shared $T)` は同じエージェント内でコピーできる性質を表し、Worker へ送れることや Rust の Send / Sync を意味しない。GC の `ref`、`box`、resource、Future / Stream の端点を atomic の値として格納することは型エラーになる。整数のアドレスと値はコピーできる。式の引数は左から1回ずつ評価し、その中の move・二重消費・未消費・return の経路も既存の所有権検査を通す。待機中のローカル所有値はそのスレッドに留まる。

通常の atomics モードの検査はアドレスごとの排他所有、送信後の領域アクセス禁止、ホストコードの競合、ロックの正当性、deadlock を証明しない。i32 のポインタを「その領域を所有する型」として扱う規則も追加していない。通常モードでは Lean のカーネルは従来の所有権契約を検査する。固定セルに制限した条件付きの証明には [atomicセル検証](atomic-cells.md) の `%race-free` を使う。オブジェクトをスレッド間で送りたい場合は、整数／バイトのプロトコルと寿命・同期の規約を別に定義する必要がある。

wait の timeout はナノ秒で、負数は無期限。戻り値は 0 = notified、1 = not-equal、2 = timed-out。JS の `Atomics.wait` / `waitAsync` / `notify` と同じ待ち行列を使う。ブラウザの main thread では同期 wait は使えない。Worker で有限 timeout または明示的な notify による終了手段を用意する。task cancellation は同期 wait を中断しない。

## 境界と検査

[WIT の型](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md)には raw memory を表す型がない。この実装ではメモリを import / export する Component / WIT 出力を `E_COMPONENT_MEMORY` で拒否する。Component 内部で定義して使うメモリは利用できる。JS への変換では jco の `multiMemory: true` を使い、エンジンの multi-memory に直接渡す。生のメモリ export は Core Wasm の `instance.exports` から取得する。生成する Core＋JSPI の api は関数のみを公開し、共有元の memory はホストが保持する。

通常の Core `--verified` はホストの memory import を `E_VERIFIED_MEMORY` で拒否する。`%race-free` では固定セル契約を検査した import を受理するが、実行時にはホストによるメモリの提供と契約の順守が必要になる。

ブラウザで共有メモリを使う配信には cross-origin isolation が必要。検査用サーバーは COOP `same-origin` と COEP `require-corp` を付ける。Playground はホストメモリの提供を行わない。

```sh
just test-threads
```

67命令の型と生成 Wasm、自然アライメント、引数型、effect、所有値、DCE、ホストとの共有、Worker の同時加算、JS ↔ Wasm の wait / notify、Core / Component の私有メモリを検査する。この命令群自体はスレッドプールを必要としない。

別の明示的なライブラリとして[汎用Workerホスト](worker-host.md)を用意した。Node・ブラウザ共通で非同期ジョブとCPUジョブを配送し、個別取消し・join・終了を管理する。[公開後不変の共有領域](readonly-regions.md)と[簡潔索引](succinct.md)はその利用例。
