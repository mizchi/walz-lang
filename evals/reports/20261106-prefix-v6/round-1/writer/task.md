# 初見のコード変更評価

読む資料は、この task.md と同じディレクトリの cheatsheet.md、contract.md、main.wlt のみ。
reader の資料・回答、counted の完成例、過去の評価、依存実装、SPEC、リンク先は読まない。
CLI check/types/fmt の診断は利用可。コンパイラが include を読むのは通常のビルドであり、手作業の実装探索とは区別する。
編集する candidate.wlt 自体も読み直せる。

candidate.wlt を編集し、成功値を read-lines が配送した行数へ変える。空行・コメント・I/O に成功した業務エラー行も数え、EOF は数えない。
元の JSON 出力、I/O err の伝播、業務エラー後の継続、取消しの終了処理を維持する。i32 の加算は折り返す。
fold の状態は pair<(ref $stock.State),i32> とし、stock.new(4096) は main の開始時に一度だけ呼んで全行で再利用する。
stream.fold-result と result.let を使う。通常の補助関数は使えるが、可変カウンタや別の状態表現で pair を回避しない。
program.wlt は変更しない。書込先は /Users/mz/ghq/github.com/mizchi/wlt-lang/evals/runs/syntax-prefix-20261106-r1/writer/candidate.wlt と /Users/mz/ghq/github.com/mizchi/wlt-lang/evals/runs/syntax-prefix-20261106-r1/writer/report.json のみ。

cwd: /Users/mz/ghq/github.com/mizchi/wlt-lang/
check: 'cargo' 'run' '--locked' '--quiet' '--' 'check' '/Users/mz/ghq/github.com/mizchi/wlt-lang/evals/runs/syntax-prefix-20261106-r1/writer/candidate.wlt'
診断と修正回数を記録し、未完なら未完とする。追加の資料探索で不足を隠さない。

report.json形式:
{"completed":false,"checks":[{"command":"...","exitCode":1,"diagnostic":"..."}],"repairs":[],"ambiguities":[],"assumptions":[],"extraDocumentsRead":[]}

実行の合否は親の固定 verifier が判定する。自己採点や completed=true だけでは合格にならない。
