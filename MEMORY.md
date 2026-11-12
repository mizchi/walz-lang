# メモリ管理と実測

Latest verification: [2026-11-12 runtime, GC, and compiler audit](docs/memory/20261112-audit-r2/README.md).
It covers both JSPI backends, both experimental native JS outputs, recent lowering,
Worker/task/I/O cleanup, and repeated Wasm compiler calls. Re-run recent cases with
`just memory-recent`; the dated report preserves the measured source hashes and limits.

GC 値の寿命は Wasm GC に委ねる。ランタイムで手動管理するのは、ABI の線形バッファと外部の readable handle に限定する。
独自の GC、参照カウント、GC オブジェクトの登録表、GC による外部 resource の暗黙 close は追加しない。
割当て回数と処理時間のチューニングは [PROFILE.md](PROFILE.md) に記録する。ここでは反復後の保持と解放を検査する。

| 領域 | 管理する層 | 終了時の規約 |
|---|---|---|
| struct/array/closure/bytes/text | Wasm GC エンジン | 到達不能になった値をエンジンが回収する |
| Canonical ABI の ptr/length と複合値 | 小さな線形アリーナ | 入力コピー後、post-return、取消し完了後に解放し再利用する |
| Component の Future/Stream | Component ABI と JS の境界 | consume/close/EOF/取消しで端点と関連する登録を外す |
| JS のタスク記録 | JSPI ホスト | 終了後は引数・取消しハンドラ・通知リスナからインスタンスを保持しない |

通常の core Wasm 出力は GC 値を標準命令で操作する。Component/JSPI の出力では jco-transpile 0.23.0 の glue を使用する。
JS 専用の Core/JSPI 出力は、同じ GC lowering・アリーナ・タスクの取消しを使い、境界の JS を直接生成する。
Wasm GC の回収をホストで再実装しない。jco の互換修正も、境界で不要になった登録と参照を外す処理に限る。

## 再現した問題と修正

Node 24.21.0 / macOS arm64 で確認した。

| 問題 | 修正前の再現 | 修正と回帰検査 |
|---|---|---|
| 終了した配送端点が jco の表に残る | Future の10回受信後、future/global 表と handle 表に各20件残る。close-only でも増える | read/close/EOF 時に関連する表から削除。返却して未受信の端点が残る対照例も検査 |
| ManagedBuffer の未使用の登録表 | lookup/delete を呼ばないのに、生成したバッファをすべて強参照する | pinned glue に lookup 呼び出しがないことを検査し、登録を除去。コピーの継続が直接持つ参照に寿命を委ねる |
| prefix stream の消費済み要素・tail | 終了した iterator を保持すると、payload と tail が回収されない | 消費したスロットを空にし、終了時に tail を切る。WeakRef と強制 GC で検査 |
| 終了済み Task がインスタンスを保持 | Task の cancel closure と、取消し例外の lazy stack の CallSite が Tasks を保持する | ハンドル生成を分離し、terminal 時に取消し処理を外す。取消し例外の stack を文字列化する。終了済みハンドルを保持した状態で Tasks の回収を検査 |
| 線形アリーナの断片化 | bytes の長さを1ずつ増やすと、長さ11561で16 MiBを使い切る | 空き領域を分割・隣接結合し、末尾を再利用。長さ1..20000の往復と、32個のランダムな同時割当て・順不同の解放を100周して内容と非重複を検査 |

返却端点のラッパーには jco の既存の端点IDを再利用し、二重登録のための追加表は持たない。
端点の表を削除しても、実行中の copy は直接のオブジェクト参照で完了する。
進行中の read の取消しは従来どおり完了通知まで待つ。古い端点からの遅い通知では、再利用した handle の別オブジェクトを削除しないよう同一性を確認する。
現在の writer はホスト所有である。言語内 writer や別の jco バージョンに進む際は、この境界の契約を再検査する。

## 同じ負荷による測定

各ケースは1バッチを warmup した後、6バッチを測定した。各サンプルはイベントループを進め、4回の強制 GC 後に取得した。
bytes/text は1バッチ1000組の ABI 往復。配送は1000組の Future 返却・受信、Stream 返却・完走、外部 Future/Stream の受信。
close-only は200個の60秒タイマー Future を開始してすぐ閉じる。

| ケース | 修正前の JS heapUsed | 修正後の JS heapUsed | 修正後の線形メモリ |
|---|---:|---:|---:|
| bytes/text | 11.12 → 11.15 MB | 11.19 → 11.25 MB | 64 KiB |
| Future/Stream | 50.36 → 143.53 MB | 12.12 → 11.72 MB | 64 KiB |
| close-only | 13.68 → 16.70 MB | 12.66 → 12.73 MB | 64 KiB |

MB は10^6 bytes。修正後の全サンプルで、アリーナの未解放数、端点/global/handle 表、buffer 登録表、task/thread/subtask/resource scope の残数は0だった。
正常終了と trap 後に破棄したインスタンスについて、Wasm Memory オブジェクトが回収されることも WeakRef で確認する。

```sh
just memory-check
```

GC 到達性・管理表・アロケータの回帰検査に続いてストレス測定を実行し、`target/memory-audit/report.json` に数値を保存する。
終了を強制しないので、対象の負荷で不要な timer が残ってプロセスが生存し続ける場合も検出できる。
検査用の `__memory` はテスト成果物にだけ追加し、通常の生成物には含めない。
通常の `pnpm test` は強制 GC に依存する2検査を skip するため、GC 到達性まで確認する場合は memory-check を使う。
heapUsed の warmup 後の増加が2 MiB以上、線形メモリが反復で拡大、管理表またはアリーナに未解放の項目が残る場合は失敗する。

## 軽いランタイムを維持する境界

Component 経由の JSPI 用 jco glue は、測定した3ケースで非圧縮約406–431 KBある。
この glue は Component の型、handle、配送・取消しの相互運用を担当する。core Wasm の実行経路へは依存を持ち込まない。
バッファ管理を減らす際も、Wasm GC の参照を linear memory のポインタに置き換えたり、外部 handle の close を GC と同一視したりしない。

## Core＋JSPI 出力との比較

2026-11-04、Node 24.21.0 / macOS arm64。同じ fixtures と上記の負荷で両出力を測定した。
JS の値は、実行時に読み込む `.js` / `.mjs` 全体の合計。共通ホスト、タスク管理、アプリ固有の bindings も含み、minify はしていない。
gzip / Brotli はモジュールごとに圧縮したサイズの合計で、Wasm・型宣言・ビルドツールは含まない。

| fixture | Component の JS | Core の JS | Component gzip / Brotli | Core gzip / Brotli |
|---|---:|---:|---:|---:|
| bytes/text | 441,036 B | 29,282 B | 75,456 / 58,611 B | 9,036 / 7,839 B |
| Future/Stream | 446,527 B | 29,365 B | 74,898 / 58,344 B | 9,059 / 7,858 B |
| event-loop | 422,969 B | 28,207 B | 75,433 / 59,375 B | 8,970 / 7,800 B |

この負荷で JS 全体は約93%小さくなった。Core 出力は private ABI であり、標準 Component の汎用的な相互運用を含む実装との機能差がある。
数値・Option/Result・unit・Future/Stream・bytes/text・取消しについては、同じ Node と Chromium の実行テストを両出力に通している。

ヒープ測定では、ビルドを別プロセスで実行して jco などのビルドツールをサンプル対象から外した。以下は各ケースの6バッチの最初と最後の強制 GC 後の heapUsed（MB=10^6 B）。

| fixture | Component | Core |
|---|---:|---:|
| bytes/text | 6.21 → 6.24 MB | 5.16 → 5.21 MB |
| Future/Stream | 8.51 → 7.43 MB | 5.54 → 5.59 MB |
| event-loop | 8.22 → 8.28 MB | 5.78 → 5.83 MB |

両出力とも全サンプルで線形メモリは64 KiB、未解放バッファと検査対象の管理表の残数は0。ビルドを同じプロセスで行った以前の表とは、ヒープの集計条件が異なる。
Core の表はゲスト所有の外部端点だけを管理する。呼び出し元へ返した端点は表から取り出すため、未受信でも管理表は0になり得る。その値の寿命と close は呼び出し元の責任として継続する。
返却した Future/Stream と、取り消した read の元の未完了 Promise を保持した状態でも、不要になった Wasm Memory が回収されることを WeakRef で検査した。
入力変換の失敗・Wasm trap・非同期 cleanup の失敗についても、失敗したタスクと例外を保持した状態で同じ検査を行う。
例外の遅延スタックが実行中の closure を保持するケースを再現し、公開前に stack を文字列化して解消した。Error.cause と AggregateError.errors の内側も処理し、元の例外の同一性を保つ。

```sh
just memory-check     # Component の到達性・管理表・アリーナと測定
just memory-core-js   # Core の到達性と、同じ負荷での管理表・アリーナと測定
```

結果は `target/memory-audit/report.json` と `target/memory-audit-core/report.json` に保存する。
Core の検査用 `__memory` もテスト成果物にだけ追加する。独自 GC や GC オブジェクトの登録表は追加していない。

## text I/O の終了処理

`just test-io` は両バックエンドで次を検査する。テストは終了を強制せず、Node CLI の子プロセスが自力で終了することも確認する。

- 分割された UTF-8 / CRLF / 空行 / 最終行、UTF-8 不正と行長上限。
- read/write の待機中の取消しと、provider の非同期 cleanup より前に終了確認を返さないこと。
- 正常終了・I/O エラー・取消し・遅い配送の後、アリーナとゲスト所有の端点登録が0になること。
- 閉じた reader を保持しても、source、chunk、source.return の不要な戻り値が回収されること。
- ファイル読み書き、空の出力によるファイル作成・切詰め、実際の SIGINT と終了コード130。

text I/O helper と Node の handle 管理・CLI は、必要な host が明示的に使う別モジュールである。上の3件のサイズ表はこれらを使用しない fixtures の生成 JS を測定している。
ファイルの open/close は Node 側で扱い、Wasm GC の root 表や独自 GC は追加しない。

## 結論の範囲

この検査で、再現した増え続ける参照保持を解消し、対象の反復・close・取消し経路で回収と再利用を確認した。
任意のプログラム・エンジン・外部 producer に対する無リークの証明ではない。既存の Lean モデルは所有集合や終了確認を検査するもので、JS の参照グラフや実装全体の意味保存までは証明しない。
今回の問いには、同じ実装の実測・保持経路の heap snapshot・回帰検査を使った。

返却された未消費の端点は呼び出し元の所有物で、消費または明示 close が必要になる。JS の Stream は return で閉じられる。
返却 Future を受信しない用途では、future.close を使うゲスト export に返す。Component 経由の JS thenable 自体には独立した dispose API はない。Core 出力の返却 Future は `Symbol.asyncDispose` で producer の dispose に委譲できる。
外部 producer が終了処理に協力しなければ、その producer の Promise や I/O を強制終了する保証はない。
ユーザーが保持する結果・取消し理由・参照先も、その所有者が手放すまでは生存する。
Wasm Memory の確保済みページとエンジンのキャッシュは、個々の値の解放とは別である。RSS が直ちに減らないことだけでリークとは判定しない。
EOF/close/取消しの完了前に正当に所有されているデータは、管理表に残っているという理由で破棄しない。


## 型付きコア拡張のメモリ回帰検査

数値命令・平坦なResult連鎖・スカラーnewtype・早期脱出・所有値と例外の併用を、専用の負荷で検査する。
`just memory-core-expressions` は両JSバックエンドのバッファ・配送表・task scope・外部資源の残数、Wasm GC値と失敗履歴の到達性、反復後の保持量を照合する。

2026-11-06の検査では、全36サンプルで未解放項目がゼロ、線形メモリは64 KiBのまま、各backendで11,200回の取得とcloseが一致した。
448個のWasm GC値も直接回収を確認した。[方法・制約・固定した測定結果](docs/memory/20261106-core-expressions-r1/README.md) を参照。
