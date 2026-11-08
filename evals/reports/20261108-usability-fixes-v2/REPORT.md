# 使い勝手の修正と再検査 — 2026-11-08 / v2

[前回の指摘](../20261108-usability-v1/REPORT.md)を1→5の順に修正し、新しい入力スナップショットで評価した。**操作チェック15/15成功**。[result.json](result.json) / [commands.json](commands.json) / [実行条件](manifest.json) に証拠を保存した。

実装者による非初見の試用であり、人間の初見評価や作業時間の改善測定ではない。v2ではエラー中のhover/definitionとcompletionの検査を追加したため、v1との主観点の差は集計しない。旧評価の入力・回答・結果・スナップショットは変更していない。

## 1. 通常ビルドとharnessの公開範囲

通常のJS/Componentビルドから、metadataで識別したtest/benchの生成exportだけを取り除く。利用者のexport名をprefixで削除しない。型・effect・所有権は全ソースで検査し、test/benchの誤った宣言も引き続き拒否する。個別runnerは明示的に選んだentryだけを公開する。

benchと同居したアプリもCore＋JSPIへビルドでき、`main`だけを通常の公開APIとして使える。Node/ブラウザの検査に加え、browser向けコンパイラでも同居するbenchが公開されないことを確認した。低水準のCoreコンパイルAPIは、既存のnative harness ABIを維持している。

## 2. ソース上の型名による診断

同じ間違いに対して、現在の診断は次の形になる。

```text
type mismatch: expected (ref $dict<i32>), got (ref $dict<i64>)
```

単相化metadataを使って型引数を復元し、エラー位置のスコープで曖昧でない別名を表示する。識別子全体を対応付けるため、`.mono1`と`.mono10`の部分一致置換は起こさない。利用者自身が書いた名前の`.mono` suffixは消さない。表示だけの処理であり、型の同一性、ABI、診断code、source spanは変えない。循環と共有DAGの展開にも上限を設けた。

## 3. エラー編集中のLSP

現在のリビジョンの宣言・参照位置を、checked IRとは別に保持する。型検査が失敗した場合のhoverには次を表示する。

```text
binding $answer (no inferred type)
Declared syntax (unverified; no checked type is inferred)
```

未検査の値をchecked typeとして返さず、以前のリビジョンの型や位置も使わない。未保存のimport先への定義移動、UTF-16、shadowing、編集による行の移動、型と関数とローカルの同名を検査した。閉じ括弧が未入力のときは、エディター専用のprefix構文木から利用可能な宣言を拾う。通常のコンパイラのparser/validatorは厳格なまま。

ローカル・明示importのcompletionと、core import内の公開API候補も追加した。sourceの自動書換えやauto-importは行わない。壊れたtokenや解決不能のimportでは回復できる範囲に限界があり、構文情報から他moduleのsymbol identityを推測しない。[LSPの契約](../../../docs/lsp.md)を参照。

## 4. 資料の同期

scoped-tasks/resources/SPEC/WASI値adapterの古い未対応記述を更新した。CLIに、Coreの値表示、Core＋JSPIのJSON値、明示stdio、native Component組込みの選択表を追加した。stringのCore表示が本文ではないこと、`--stdio`が戻り値のraw text変換ではないことも明記した。

## 5. 文字列とOption/Result

期待型がsemanticなstringと決まる位置では、裸のUTF-8文字列を `%string.lit` と同じIRにする。引数・結果・型付きlet・フィールド・listに適用し、期待型がない場合は型注釈か明示形を求める。他の型への暗黙変換は行わない。明示形と同一のWasmを確認した。`%string.len`の診断は元の名前を表示し、`%string.byte-len`を案内する。

`$option.ok-or`と`$result.map-err`は既存の仕組みで単相化する通常のcore関数として追加した。error引数のeager評価、err時だけのcallback呼出し、callback effectの伝播、GC参照の同一性、ABI解放を両backendで検査した。payloadはshared/nonunitに限定し、unitや所有値には既存のmatchを使う。[仕様と例](../../../docs/option-result.md)。

JSON課題ではローカルのOption→Result helperをcore関数に置き換え、期待型のある文字列を省略形にした。oracle入力と集計の意味は維持し、両backendと方針変更版で照合した。整形済みの本体・parser・入口は合計197行から182行、parserの最大S式深さは7から6になった。このコードの計測値であり、人間の読み書きが同じ割合で改善したという意味ではない。[修正後のparser](snapshot/expenses/parse.wlt)。

## 検査

| 検査 | 結果 |
| --- | --- |
| 開発体験の操作 | 15/15 |
| Rust workspace lib/tests | 520成功、native engineの専用検査1件は既定でignore |
| Node runtime（GC有効） | 144成功 |
| Chromium runtime / Playground | 75 / 51成功 |
| Lean契約 | 5ファイル成功 |
| 言語テスト / eval harness | 47 / 19成功 |
| fmt / clippy / 生成API資料 | 成功 |
| WIT roundtrip | 成功 |
| ソースを除去したpackageのCLI検査 | build/run/test/bench成功 |
| 専用のWasmtime native async検査 | 2成功 |

各logをこのディレクトリへ保存した。新規の可読性点数を実装者だけで付け直して成功の根拠にすることは避け、操作・コード・実診断の変化を記録した。全体のJSPI実装やLSP回復の形式的意味保存を証明したものではない。
