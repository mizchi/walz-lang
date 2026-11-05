# 初見の読解・可読性評価

読むものは、この task.md と同じディレクトリの cheatsheet.md、contract.md、rubric.md、main.gwat、counted.gwat のみ。
リンク先、依存実装、他の役、過去の評価、会話履歴は参照しない。コンパイラや実行ツールは起動しない。
書込先は {{READER_OUTPUT}} のみ。他のファイルを変更しない。

先に main の処理順、counted の状態型と成功値、alias があれば実行時 identity の有無、pair 分解と move の関係、業務エラー/I/O err/EOF/取消し、result.let が関数からの return かを説明する。
カウンタの型や更新処理を変えるときに追う箇所も具体的に述べる。未知の構文があれば推測で埋めず明記する。

その後 rubric.md の5軸を0〜4で採点し、各々具体的な行を根拠にする。正しく説明できることを「読みやすい」と評価しない。
改善案は syntax/format/naming/docs に分ける。実行結果や読了時間を推測しない。

JSON形式:
{"answers":{"flow":"...","state":"...","alias":"...","ownership":"...","errors":"...","scope":"...","changes":"..."},"scores":{"flow":{"score":0,"evidence":"..."},"types":{"score":0,"evidence":"..."},"scope":{"score":0,"evidence":"..."},"layout":{"score":0,"evidence":"..."},"changes":{"score":0,"evidence":"..."}},"issues":[],"limitations":[]}

issue の形式は {"priority":1,"kind":"syntax|format|naming|docs","location":"...","problem":"...","fix":"..."}。
全軸3以上が目標だが、無理に合格させない。
