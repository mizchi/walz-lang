# 在庫台帳サンプルの外部契約

サンプルの `program.gwat` は既存モジュールを静的に読み込む。課題では次の契約だけ使い、実装は読まない。

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

配布される writer ディレクトリに program.gwat の橋渡しファイルを親が置くので、候補は `(include "program.gwat")` のまま書く。
作業ディレクトリは `/Users/mz/ghq/github.com/bytecodealliance/wasmtime/examples/gcwat`。
`cargo run --locked -- check <候補のパス>` で検査できる。コンパイラが依存ファイルを読むのは通常のビルドであり、手作業で実装を読むこととは区別する。

配布版の末尾には、このラウンド固有の候補パスと入力付きの起動コマンドを記載する。
ビルドは `node runtime/build-core.mjs <候補> <出力ディレクトリ>`、実行は `node runtime/text-cli.mjs <出力ディレクトリ> --input <入力ファイル>`。
stdin を使う場合は `--input ...` を省略する。成功値の行数はプロセスの exit code にはせず、main の返す Result の中にある。

この配布物の候補: /Users/mz/ghq/github.com/bytecodealliance/wasmtime/examples/gcwat/target/grammar-eval/round-2/writer/candidate.gwat

作業ディレクトリを上記 gcwat にして実行:

```sh
cargo run --locked --quiet -- check /Users/mz/ghq/github.com/bytecodealliance/wasmtime/examples/gcwat/target/grammar-eval/round-2/writer/candidate.gwat
node runtime/build-core.mjs /Users/mz/ghq/github.com/bytecodealliance/wasmtime/examples/gcwat/target/grammar-eval/round-2/writer/candidate.gwat /Users/mz/ghq/github.com/bytecodealliance/wasmtime/examples/gcwat/target/grammar-eval/round-2/run
node runtime/text-cli.mjs /Users/mz/ghq/github.com/bytecodealliance/wasmtime/examples/gcwat/target/grammar-eval/round-2/run --input examples/apps/stock-ledger/sample.tsv
```
起動は参考手順。評価役が利用してよいツールの範囲は各 task の制約に従う。
