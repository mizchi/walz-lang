# 初見のコード変更評価

読む資料は、この task.md と同じディレクトリの cheatsheet.md、api.md、contract.md、main.walz のみ。
reader の資料・回答、counted の完成例、過去の評価、依存実装、SPEC、リンク先は読まない。
CLI check/types/fmt の診断は利用可。コンパイラが include を読むのは通常のビルドであり、手作業の実装探索とは区別する。
編集する candidate.walz 自体も読み直せる。

candidate.walz を編集し、成功値を read-lines が配送した行数へ変える。空行・コメント・I/O に成功した業務エラー行も数え、EOF は数えない。
元の JSON 出力、I/O err の伝播、業務エラー後の継続、取消しの終了処理を維持する。i32 の加算は折り返す。
fold の状態は pair<(ref $stock.State),i32> とし、stock.new(4096) は main の開始時に一度だけ呼んで全行で再利用する。
stream.fold-result と result.let を使う。通常の補助関数は使えるが、可変カウンタや別の状態表現で pair を回避しない。
初期化では複数束縛の @let を少なくとも一度使い、型の明示参照は walz:core から import した名前と @inst を使う。
同じ candidate.walz に次の公開関数も追加する。
- `count64`：行数カウント版の成功型を i64 にしたもの。関数 `$count64` を宣言し `(export "count64")` を付ける。fold の状態も pair<(ref $stock.State),i64> とし、初期値・加算・定数を i64 に変更する。I/Oエラーは i32 のまま。i32 の最終結果を変換するだけでは不可。
- `format-count`：関数 `$format-count` を宣言し `(export "format-count")` を付ける。引数は text 1つ、結果は Result<text,i32>。符号なし64bit整数の十進数全体を読み、成功なら `count=数字`（先頭ゼロを除く）を ok、空・負数・空白・不正文字・桁あふれなら err(1)。標準APIを選択して実装する。I/Oやホスト呼出しは不要。APIの型とeffectは api.md、解析の入力規約は下記の契約に従う。
解析は ASCII 数字だけを認め、`+` は認めない。範囲は0〜18446744073709551615。`count64` も在庫初期化は関数開始時の1回とし、JSON出力・I/O err・終了処理を維持する。

program.walz は変更しない。書込先は {{CANDIDATE}} と {{WRITER_OUTPUT}} のみ。

cwd: {{PROJECT_ROOT}}
check: {{CHECK_COMMAND}}
診断と修正回数を記録し、未完なら未完とする。追加の資料探索で不足を隠さない。

report.json形式:
{"completed":false,"checks":[{"command":"...","exitCode":1,"diagnostic":"..."}],"repairs":[],"ambiguities":[],"assumptions":[],"extraDocumentsRead":[]}

実行の合否は親の固定 verifier が判定する。自己採点や completed=true だけでは合格にならない。
