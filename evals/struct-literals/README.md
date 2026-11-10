# struct構築表記の読解評価

位置引数、名前付きS式、波括弧＋明示リテラル、波括弧＋期待型リテラルの4案を比較する。
現在は[名前付き構築](../../docs/struct-construction.md)として4表記を実装している。導入前のv1/v2では候補構文として評価した。
現在のprepareはv8で、`@` 接頭辞を使い、`walz:core`から`@use`し、ファイル単位でモジュールを分ける。v5で標準型名を`$string`、文字列命令を`%string.*`、短縮形を`%$Type::`に統一した。過去の入力・結果は書き換えない。v1/v2と実装確認v1では旧綴り`%Type::`、v3では型名`$text`、v4までは文字列命令`%text.*`を使っていた。
コンパイルできたことやソースの短さを可読性の加点に使わない。

6ケースはUser、同型の数値フィールド、nestedな集計、記述順と配置順が異なる初期化、mutableなCell、実際のworkflow.Jobを参考にした構築。
各案について6つの読解回答と、Bucket.totalだけを100に変える変更課題を出す。
回答と編集式の照合は可読性5軸の主観点と別に扱う。編集式は元の表記を保った参照式との空白を除く一致を検査するため、別の等価な式を提出した場合は人が監査する。

```sh
just eval-struct-prepare evals/runs/struct-trial-01
```

作成された`readerA/packet.md`、`readerB/packet.md`を、それぞれ会話履歴を持たない別のエージェントへ渡す。
読取りは各packetだけ、書込みは`readerA.json`、`readerB.json`だけに限定する。
packet末尾のschemaで回答させ、oracle・manifest・他方のpacket・過去の点数を渡さない。
R/S/T/Uの提示順と対応を逆にし、順序による評価の偏りを確認する。

```sh
just eval-struct-verify evals/runs/struct-trial-01 \
  evals/runs/struct-trial-01/verification.json
just run evals/runs/struct-trial-01/baseline.walz
just test-evals
```

prepareと検証結果の保存は既存の出力先を上書きしない。配布物を変えたら新しいrunを作る。
source・rubric・oracle・packetをhashとともに固定し、verifyはrun内の固定した実装を使う。
baselineは現行構文だけで6ケースの観測を実行し、成功時に42を返す。候補構文をコンパイルしたとは扱わない。

[v1](../reports/20261106-struct-literals-v1/REPORT.md)では資料の`@let`説明不足が判明した。
規則を補足した[v2](../reports/20261106-struct-literals-v2/REPORT.md)を新しい読解役で評価している。
少数の同系統LLMによる主観評価であり、人間の読了時間・変更速度・誤答率、統計的な有意差を測った結果ではない。
