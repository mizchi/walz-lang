# 親による回答の監査

これは初見の採点役へ渡さない。reader.json の回答を、配布したコード・契約・チートシートへ照合する。
型、評価順、所有性、EOF/err/取消し、Result の成功値と業務エラーについて誤読がないか確認する。
answers.syntax は、標準 WAT と拡張の入口、結果宣言と Result 値型、静的リンクと Wasm import/export、型適用と具体化後の検査、shared/pure の範囲、opaque 解析の制限をチートシートへ照合する。接頭辞が内部の case/bind にも必要、shared が不変、pure が無 trap、opaque でも effect/所有権が検査済み、といった誤読を見落とさない。
answers.bindings は左から右に一度ずつの初期化と既存の消費義務、answers.standard は標準名の明示 import と Result/Option/Future の共通の型適用を確認する。固定構文を型名の宣言と混同していないかも確認する。
scores の根拠が具体的か、実行成功を可読性へ加点していないかも確認する。型情報を省略しただけで満点にしない。

正答なら次を実行し、理由を具体的に記録する:
'node' '/Users/mz/ghq/github.com/mizchi/wlt-lang/evals/run.mjs' 'review' '/Users/mz/ghq/github.com/mizchi/wlt-lang/evals/runs/standard-bindings-20261106-r1' '--correct' '--reason' '型・制御・所有性の回答を配布コードの具体的な箇所に照合した理由'

誤りがあれば --correct を --incorrect に変える。review は回答の SHA-256 を固定する。
回答を修正した後は古い review を使わず、再監査する。書いた本人の自己申告だけで監査を省略しない。

最後に実行検証と集計:
'node' '/Users/mz/ghq/github.com/mizchi/wlt-lang/evals/run.mjs' 'verify' '/Users/mz/ghq/github.com/mizchi/wlt-lang/evals/runs/standard-bindings-20261106-r1'
'node' '/Users/mz/ghq/github.com/mizchi/wlt-lang/evals/run.mjs' 'score' '/Users/mz/ghq/github.com/mizchi/wlt-lang/evals/runs/standard-bindings-20261106-r1'

summary の pass は、全5軸3以上・回答の監査成功・固定実行チェック全成功のすべてを要する。
