# 診断の初見評価

読むものは、この task.md、rubric.md、cheatsheet.md、cases/ 内の intent.md・診断・ソースだけ。
compilerの実装、writer、corpus、private、過去の評価、リンク先、会話履歴は読まない。コンパイラを起動しない。
書込先は {{READER_OUTPUT}} のみ。

全ケースについて、まず原因、意図を保つ修正案、診断の説明を記す。診断だけから分かることと、コード/資料から補ったことを分ける。
その後5軸を0〜4で採点する。分からなければ分からないと書き、無理に合格させない。

JSON形式:
{"cases":{"case-id":{"cause":"...","fix":"...","explanation":"...","scores":{"location":{"score":0,"evidence":"..."},"cause":{"score":0,"evidence":"..."},"expectation":{"score":0,"evidence":"..."},"repair":{"score":0,"evidence":"..."},"focus":{"score":0,"evidence":"..."}}}},"issues":[],"limitations":[]}

casesのキーは配布されたすべてのcase-id。issueは {case,axis,problem,suggestedFix}。
全文一致や文の長さを品質の代用にしない。読了時間や修正成功を推測しない。
