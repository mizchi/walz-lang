# 開発体験の試用 v1

対象はWAT/Wasmを理解する開発者。実装を知る評価者による非初見の試用であり、人間の初見試験や作業時間の測定として扱わない。
コンパイラ・ランタイム・既存の説明を評価中に修正せず、失敗したソースと診断を別のattemptとして保存する。過去の評価点との比較はしない。

1. README の例をそのまま check/build/run する。
2. JSON行からcategoryとsigned64bitのamountを取り出す。入力エラーをenumにし、正常行はcategory別に集計する。加算のoverflowはその行だけ拒否し、以前の集計を保つ。accepted/rejected/totalsをJSONで返す。モジュール分割、型付きResult連鎖、ジェネリクス、mutable GC、コレクションを使う。独立したJS oracleと通常・不正・Unicode・境界・大量入力を照合する。
3. 上の処理をwlt test/bench/runとJS APIから使う。小さな仕様変更（負数を返金として許す→負数を拒否する）の差分も作り、変更箇所を記録する。
4. 子タスクのtimeoutとcancelを使い、兄弟が続行することを確認する。両JS backendで同じソースを動かす。
5. エディター相当のstdioクライアントでLSPを起動し、未保存モジュール、hover/definition、間違いの導入と修正を試す。応答時間はローカルの参考値で、編集者の作業時間とはしない。
6. WITからresource bindingを生成し、borrowしてからcloseするnative Componentを作る。生成された雛形をどう接続するかを評価し、ホストのdestructor回数を確認する。

機械的な合否と、書きやすさの所見を別に報告する。機能が動いたことを可読性へ加点しない。書けなかったケースや資料探索も結果として残す。
