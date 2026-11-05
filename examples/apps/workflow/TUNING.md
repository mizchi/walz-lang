# ワークフローのチューニング（2026-11-05）

言語仕様や公開プロトコルを変えず、計画アルゴリズム・ジョブ検索・Component の再開スケジューラーを改善した。
出力を固定した参照実装、取消し・解放・GC 到達可能性のテストを通し、保存した実行形式を交互に測定した。

## 変更前後の比較

Node v24.21.0、Apple M5。各版を1回 warm-up し、順番を交互に3回ずつ実行した中央値。単位は ms。
コンパイル・instantiate・参照実装による照合・assert の時間は含めない。各回で値・JSON の全行・入力の終了・ABI 割当て0を確認している。

| バックエンド | 処理 | 変更前 | 最終版 | 比率（前/後） |
|---|---|---:|---:|---:|
| core | 3,003入力行・固定2ジョブ | 3.283 | 3.451 | 0.95x |
| core | 4,400入力行・2,200ジョブ・2,201出力行 | 90.600 | 17.202 | 5.27x |
| core | I/Oなし・2,200ジョブの構築と計画 | 71.484 | 0.676 | 105.76x |
| core | I/Oなし・65,536辺と容量検査 | 22.062 | 7.270 | 3.03x |
| component | 3,003入力行・固定2ジョブ | 3791.643 | 104.772 | 36.19x |
| component | 4,400入力行・2,200ジョブ・2,201出力行 | 8362.468 | 218.619 | 38.25x |
| component | I/Oなし・2,200ジョブの構築と計画 | 73.948 | 0.691 | 107.01x |
| component | I/Oなし・65,536辺と容量検査 | 21.579 | 6.460 | 3.34x |

2,200ジョブの計算は約100倍、Component の入力処理・計画出力は約36〜38倍に改善した。
小さい Core 入力の中央値は約5%遅く、観測範囲も重なる。このケースで安定した改善は確認していない。
少数のローカル測定であり、機種や入力全般に対する倍率の保証ではない。

全量 I/O の別検証では、65,795入力行を Core 767.3 ms、Component 4056.6 msで処理し、両方の結果が一致した。
これは warm-up をそろえた上表の比較とは別の単発値。
Core のゲスト Wasm は 17041 → 19533 byte。JS glue や memory.wasm 等は別。

## 計測から選んだ変更

### 候補の全探索を、入次数と min-heap に置換

変更前の Wasmtime ゲストプロファイルでは planner.find が inclusive 89.4%を占めた。
各イベント・空きワーカーごとに、全ジョブと依存先の完了状態を調べ直していた。

最終版は計画ごとに入次数と逆向きの依存配列（CSR）を作り、完了時に影響するジョブの入次数だけを減らす。
0になったジョブを min-heap に入れ、最小の宣言番号を取り出す。
FIFO にすると、後から解禁された宣言順の早いジョブより後続ジョブを先に動かしてしまうため、順序を保つヒープを使った。
同じ時刻の完了をすべて確定した後に次の割当てを行う規則も維持した。

計画部分は O(V+E) の準備と、O(V log V + E + VW) の処理になる（W≤32）。
入次数・CSR・ヒープには追加の O(V+E) メモリを使う。作業用 Work から結果 Run への一方向の参照にし、結果から作業配列を保持しない。
最大 GC ヒープ使用量は未測定。

### ジョブ名検索をハッシュ索引へ変更

graph.find の全配列走査を、容量に合わせて拡張するバケット索引へ変更した。
通常の名前では検索を短くできる。衝突時は連結をたどるので、最悪計算量が一定になる保証はない。
既存の宣言番号・Job オブジェクト・依存辺は維持し、配列の拡張と再ハッシュ後も既存の参照を使える。

最初の候補では Job に索引用 next を置いたが、衝突した別ジョブまで保持することを WeakRef と実 GC で検出した。
最終版は索引用 Entry を別オブジェクトに分離した。Job 自身は索引を参照せず、依存関係にない別ジョブを保持しない。
このテストの Red → Green を確認し、最終版の再キャプチャ・再測定まで実施した。

### 再開のたびの setTimeout(0) を、実タスクターンへ変更

pinned jco-transpile 0.23.0 の再開ループが、再開後ごとに setTimeout(0) を待っていた。
同じ2,003行を CPU プロファイルとタイマー計数で調べると、0ms タイマーは 3011 → 61 回になった。

Node は setImmediate、ブラウザーは MessageChannel で実タスクターンを待つ。
これによりマイクロタスクを流し終えてから次のスレッドを再開する性質と、1 tick に1スレッドという規則を保つ。
IDLE の10ms polling と、明示的な初回・取消し等の待機は維持した。対応 API がない環境は元の timer fallback を使う。
単なる queueMicrotask の連鎖にはしていない。

MessageChannel は生成モジュールのスコープに置き、component instance を捕捉しない。保留 callback は次のターンで取り除く。
pinned 版のコードが変わったら、パターン不一致でビルドを止める。

## 検証

- Rust 全テストと fmt・clippy 成功。
- Node 全43件を GC 有効で実行し、43成功・skip 0。
- Playwright 24件成功。ブラウザーの実 MessageChannel 経路、取消し、GC 参照の中断・再開、ワークフローの優先順・循環を確認。
- 言語内モデル12件、ヒープ3件。逆順挿入、途中で追加された小さい番号、同値、空・単一要素を確認。
- 両バックエンドの17入力群を参照実装と照合。65,536辺、容量超過、再ハッシュ、ゼロ時間、同時完了、再計画、計画後の更新、UTF-8、I/O 失敗も含む。
- 読み書き中の取消し、非同期解放完了後の通知、同じ instance の20回再利用、所有 handle・scope・ABI 割当て0を確認。
- Job の GC 到達可能性の回帰テストを追加。依存していないジョブの寿命を索引が延ばさないことを確認。
- 評価基盤11件成功。既存の可読性・診断評価用の採取と検証も維持した。

変更前のゲスト profile は 31246 samples、最終版は 63 samples。
最終版は処理が短いため、詳細な関数順位は参考程度。速度比較には profiler を動かしていない上表の結果を使う。

## 再現方法と保存物

examples/gcwat を作業ディレクトリにする。

```sh
just workflow-capture target/workflow-tune/new-before
# ソースを変更
just workflow-capture target/workflow-tune/new-after
just workflow-profile target/workflow-tune/new-before
just workflow-profile target/workflow-tune/new-after
just workflow-compare target/workflow-tune/new-before target/workflow-tune/new-after 3
just test-workflow
just test-workflow-stress
```

capture は既存ディレクトリを上書きせず、実行ファイル・glue・text I/O provider・参照実装・gwat ソースを保存する。
profile / compare は保存した SHA-256 を検査するため、比較中に現在のソースをビルドし直して混ぜることはない。
CPU profile は component.cpuprofile、Wasmtime の guest profile は guest.json、要約は profile.json に保存する。

今回の採用版は target/workflow-tune/after-final。after は GC 保持範囲を修正する前の中間候補で、採用版ではない。
[機械可読の記録](tuning.json)には全サンプル、プロファイル要約、artifact hash、全量負荷の観測と検証件数を残した。
生プロファイルと実行形式は git 管理外の target/ にあり、上の手順で新しい比較を作れる。
初回実装の単発計測は [ASSESSMENT.md](ASSESSMENT.md) にそのまま残した。
