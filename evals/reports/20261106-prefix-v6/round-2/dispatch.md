# 評価役へ渡す指示

各役は別の初見コンテキスト。会話履歴、過去の結果、他の役の出力を渡さない。

- 読解役: /Users/mz/ghq/github.com/mizchi/wlt-lang/evals/runs/syntax-prefix-20261106-r2/reader/task.md を読み、その指示だけに従う。
- 変更役: /Users/mz/ghq/github.com/mizchi/wlt-lang/evals/runs/syntax-prefix-20261106-r2/writer/task.md を読み、その指示だけに従う。

親は両方が完了した後 reviewer-task.md を使う。このCLI自体はエージェントを生成しない。
