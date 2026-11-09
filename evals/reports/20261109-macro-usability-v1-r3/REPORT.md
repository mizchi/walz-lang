# マクロ実用性評価 v1・再実行 r3

既存の必須検査は **20/20 成功**、探索課題の未達は **0件**。前回 r2 と同じ42入力、同じ処理系バイナリ・実装で再実行した。今回、コンパイラとランタイムは変更していない。

JSON を処理するアプリの oracle・ポリシー変更・境界入力、CLI run/test/bench、Core＋JSPI／Component の ABI 解放と取消し、型・effect・所有権の拒否、stdio LSP が成功した。束縛の委譲・反復、未完の imported macro の補完、256展開までの既存スケール課題も成功した。

| 同じ入力での計測 | 前回 r2 | 今回 r3 |
| --- | ---: | ---: |
| 256マクロ展開の check 中央値 | 41.79ms | 40.99ms |

短い反復計測の差であり、処理系の高速化とは判断しない。今回のアプリの macro 版 check 中央値は55.48ms。CLI 起動・読み込みを含む。

型引数推論が導入された現在、既存アプリの型引数を補うマクロは通常関数でも置き換えられる。古い入力やルーブリックは書き換えず、その比較と追加の限界課題を [v2 の別評価](../20261109-macro-authoring-v2-r2/REPORT.md)に分けた。

## 保存した証拠

- [result.json](result.json)：全検査・診断・反復時間。
- [assessment.json](assessment.json)：既存条件の集計。
- [inputs.json](inputs.json)／[snapshot](snapshot/)：固定した入力とハッシュ。
- [commands.json](commands.json)／[work](work/)：実行記録、Core／Component 成果物、LSP 結果。
- [manifest.json](manifest.json)／[verification.json](verification.json)：同一バイナリ・274実装ファイルと既存2,830ファイルの整合性確認。

評価者は実装者本人であり、独立した読者評価ではない。v2 とは課題が異なるため、ルーブリックの点差を比較しない。
