# エラーメッセージを改善する評価ループ

正しい拒否と、診断を読んで原因・修正が分かることを別に評価する。
[10ケース](suite.json)のソース、意図、正常対照、非公開テストと[5軸の基準](rubric.md)を固定して繰り返す。
prepare ごとに現状の診断を記録し、結果から実装を改善する。
The current suite is v13 with bare Watz `case`/`default`; [v12](versions/v12.json) is preserved; [v11](versions/v11.json) is preserved; [v10](versions/v10.json) is preserved; [v9](versions/v9.json) and [v8](versions/v8.json) are preserved. Match arms use `@case`/`@default`. It uses `@` extensions, `.walz` files, and `walz:core`. The cases retain thread effects and atomic-cell verification in their reference material. The preceding percent-prefix suite is [v7](versions/v7.json), the earlier WLT suite is [v6](versions/v6.json), and older inputs remain archived.
compiler の[診断データ契約](../../docs/diagnostics.md)は表示から独立しているが、この評価では利用者が実際に読む CLI の表示を使う。

| ケース | 確認すること |
|---|---|
| type-mismatch | 実際と期待する数値型・演算の区別 |
| unknown-local | 未定義のローカル名とスコープ |
| call-arity | 関数の必要な引数数と実際の個数 |
| use-after-move | 消費済みのownerと先行/後続操作の位置 |
| match-exhaustiveness | 足りないcaseと対象の型 |
| alias-cycle | 循環するaliasと定義位置 |
| effect-contract | 宣言した作用と必要な作用の違い |
| include-location | include元ではなく実際のmath.walzの問題位置 |
| unclosed-delimiter | 閉じられていない括弧と開始位置 |
| opaque-representation | 公開APIと非公開の表現操作の区別 |

## 準備・評価・修正

`walz-lang` で実行する。Node 24以降、Cargo、Wasm GC対応Wasmtimeが必要。
Cargoが報告する実際のwalz実行ファイルを使用するため、CARGO_TARGET_DIRや作業ディレクトリの深さに依存しない。

```sh
just eval-diagnostics-prepare errors-01
# または任意の場所へ
node evals/run.mjs prepare errors-02 --suite diagnostics --output /tmp/walz-errors-02
```

prepareはcompilerをビルドし、各badソースにcheckを実行する。診断のstdout/stderr、終了状態、ソースと基準を評価回へ保存し、SHA-256で固定する。
型検査の終了コードや文言を模擬しない。panic・timeoutを通常の型エラーとして評価しない。
端末の色とマシン固有のパスだけを正規化し、原因や修正案を診断へ書き足さない。
元のパスとcanonicalパスを両方扱い、include先のファイル名・行・列を維持する。

生成したdispatch.mdを使い、別の初見コンテキストへ渡す:

- **読解役**: reader/task.md、配布ソース・意図・診断・cheatsheet・rubricだけを読む。先に原因/修正案/説明、次に各ケースの5軸を採点しreader.jsonへ書く。
- **修正役**: writer/task.mdと配布診断を使い、指定のファイルだけを修正する。意図とAPIを保ち、check/types/fmtの診断と試行回数をwriter/report.jsonへ記録する。
- **親**: reviewer-task.mdで回答を監査する。正常対照や非公開テストを初見の役へ渡さない。

読み書きの範囲は評価指示であり、OSのsandboxではない。CLIはエージェントを生成しない。
配布先のIDはcase-01等の匿名名にし、原因の分類名をディレクトリ名から推測させない。分類と正常対照は親だけが参照するsuite/corpusに残す。

```sh
node evals/run.mjs review errors-01 --correct --reason '全ケースの原因と修正案を意図・ソース・診断に照合し、診断にない情報で満点にしていないことを確認'
# 誤答なら --incorrect
just eval-verify errors-01
just eval-score errors-01
```

verify/score/reviewはmanifestからsuiteを判別する。既存の可読性評価を壊さない。
summaryの状態・終了コードは共通で、pass=0 / fail=1 / incomplete=2 / invalid=1。

## 合格条件

主観品質は **全ケースの全5軸（location/cause/expectation/repair/focus）が3以上**。平均や総点で弱い1ケースを隠さない。
正しいコードを推測して書けたことだけでは、メッセージに必要な情報があるとは判定しない。親の回答監査も必須。

機械検証はケースごとに次を確認する:

1. badソースを通常のエラーとして拒否し、panicしない。
2. 報告されたファイル・行・列が実在し、定義した問題位置の行を示す。循環など複数の妥当な位置は許可する。期待位置はソース内のmarkerから求める。
3. 正常対照が型検査と非公開の振る舞いテストを通る。
4. 修正コードも同じテストを通り、編集不可のファイル・型契約・所有値の消費等を削除していない。

正常対照も通らない場合は、ケース/ツールチェーンの不整合として検証結果から区別できる。
ownerを作って消費する処理やaliasをすべて削除する修正を避けるため、対象関数の構文上の条件と変更不可ファイルも検査する。
非公開テストは複数の値を使い、関数の削除や定数を返すだけの修正を拒否する。有限の振る舞いテストであり、完全な意味の証明ではない。
追加ファイルや外部includeによる修正は対象外。全修正ファイルをハッシュへ結び付ける。

## 改善して比較する

診断の情報不足を修正したら、**新しいprepareから別の初見評価**を行う。

```sh
just eval-diagnostics-prepare errors-02
# 新しい評価役・修正役で同じ手順
just eval-compare errors-01 errors-02
```

同じ評価回で診断文が変わった場合、古いreaderの採点を新しい文言へ流用せずincompleteにする。
診断の全文一致は品質の目標ではない。この一致確認は同一回の採点対象が変わっていないことを確認するだけで、前後の改善では文言が変わってよい。
回答・修正ファイル・実装が更新された場合も、古い監査/実行結果を使ってpassにしない。
比較時はケース/基準/corpusのfingerprintが同じときだけ、ケース別・軸別の点差を出す。基準やケースを変えたらversionを上げる。
少数モデルの主観点なので、点差を人間の修正時間や統計的な効果とは扱わない。

保存される主なデータ:

```text
reader/cases/<id>/     # badソース、intent、実際のdiagnostic.json
writer/cases/<id>/     # 修正対象、同梱の変更不可ソース
corpus/<id>/          # 親だけが見るbad/fixedとテスト仕様
private/drivers/      # 正常対照/修正用の固定テスト
manifest.json         # suite、役、compiler binary/実装/corpus fingerprint
inputs.json           # 配布物と正常対照/テストを含む固定ハッシュ
reader.json           # 各ケースの原因/修正案/説明/点数/根拠
review.json           # 親の監査と回答ハッシュ
verification.json     # 元コード・位置・正常対照・修正・振る舞いの結果
summary.json          # 機械検証と品質の別々の合否
```

`just test-evals` で、実コンパイラによる採取、正常対照、修正、消費処理削除の拒否、位置、改変、弱い1軸、更新後の失効を回帰検査する。
