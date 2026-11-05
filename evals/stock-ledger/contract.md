# 在庫台帳サンプルの外部契約

サンプルの `program.wlt` は既存モジュールを静的に読み込む。課題では次の契約だけ使い、実装は読まない。

| 名前 | 型・動作 |
|---|---|
| `$stock.State` | 共有できる non-null GC 参照で使う opaque 型。Stream 等の所有値ではない |
| `$stock.new` | `(i32) -> (ref $stock.State)`。指定の最大品目数で空の台帳を作る。allocates |
| `$stock.size` | `((ref $stock.State)) -> i32`。登録済みの異なる品目数。reads |
| `$read-lines` | `() -> (stream (result text i32))`。1要素は入力の1行。空行と # 行も配送される。host |
| `$app.process` | `((ref $stock.State), text) -> (result unit i32)`。行を処理し、必要な JSON 行を出力する。reads/writes/allocates/host/suspends |

ADD / TAKE / SHOW の処理、品目名の検査、JSON 出力は app.process が担当する。
在庫不足・容量超過・構文不正は JSON のエラーレコードを出して、I/O が成功すれば ok(unit) を返す。
したがって app.process の err(i32) は I/O 失敗であり、業務エラーの件数を返すものではない。
空行と # 行の app.process は出力せず ok(unit)。成功時に State は再利用する。

元の main は成功なら ok(0)、I/O 失敗なら err(code)。取消しは main の通常の Result 値とは別の task scope が扱う。
今回の変更課題は成功値だけを行数に変える。カウントは Wasm i32 の折り返し規則を使い、新しい整数型を導入しない。

この契約は各評価回のコードと一緒に固定する。include先の依存ソースは評価回のsupportへコピーする。
型検査のコマンドと編集先はwriter/task.mdにある。通常の補助関数は使えるが、台帳参照と行数をpairの状態に持つ条件は維持する。
main-countedの成功値はmainのResultの中にあり、text-cliのexit codeではない。観測方法は配布チートシートのrunTextProgramの例を使う。
