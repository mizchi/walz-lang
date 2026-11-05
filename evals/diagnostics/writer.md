# 診断からコードを修正する

読むものはこのtask.md、cheatsheet.md、cases/内のintent.md・diagnostic.json・編集対象と同梱ソースだけ。
compilerの実装、reader、corpus、private、正解コード、非公開テスト、過去の回答は読まない。
intent.mdの意図と公開APIを保って修正する。check/types/fmtの診断は使用可。非公開テストは親が実行する。

cwd: {{PROJECT_ROOT}}
各caseの編集可能ファイルはintent.mdで指定する。それ以外を変更しない。型・所有権・effect・opaque等の検査やAPIを削除して通さない。
新しいファイルや外部includeは追加しない。includeのファイル名は配布されたリテラル表記のままにする。
追加のコンパイル診断、修正回数、推測、メッセージだけでは不足した情報を記録する。最終的に通らなければ未完とする。
書込先は指定ファイル群と {{WRITER_OUTPUT}} のみ。

report.json形式:
{"completed":false,"checks":[{"case":"...","command":"...","exitCode":1,"diagnostic":"..."}],"repairs":[{"case":"...","changes":"...","attempts":1}],"ambiguities":[],"assumptions":[],"extraDocumentsRead":[]}

チェック例: cargo run --locked --quiet -- check <caseのprogram.gwat>
自己申告だけで修正成功とは判定しない。
