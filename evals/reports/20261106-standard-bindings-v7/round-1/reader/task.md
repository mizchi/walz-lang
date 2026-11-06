# 初見の読解・可読性評価

読むものは、この task.md と同じディレクトリの cheatsheet.md、contract.md、rubric.md、main.wlt、counted.wlt のみ。
リンク先、依存実装、他の役、過去の評価、会話履歴は参照しない。コンパイラや実行ツールは起動しない。
書込先は /Users/mz/ghq/github.com/mizchi/wlt-lang/evals/runs/standard-bindings-20261106-r1/reader.json のみ。他のファイルを変更しない。

先に main の処理順、counted の状態型と成功値、alias があれば実行時 identity の有無、pair 分解と move の関係、業務エラー/I/O err/EOF/取消し、result.let が関数からの return かを説明する。
カウンタの型や更新処理を変えるときに追う箇所も具体的に述べる。未知の構文があれば推測で埋めず明記する。

続いて answers.syntax に、チートシートだけを根拠として次を説明する。各項目に参照した節またはコードを挙げる。
- if / %let、case / bind に接頭辞を付ける規則。
- (result i64)、(result i32 i64)、(%inst $result i32 i64)、(result (%inst $result i32 i64)) の違い。
- import/export と %import/%export の役割、ファイルパスを持つ入口。
- %type.params / %where / %inst がいつ何を検査するか。関数・型・alias の適用形も書く。
- shared と不変性、%effects pure と停止・trap の関係。
- 標準 WAT、型付きコア、コア外の WAT と構文マクロの混在で、得られる解析と保証の範囲。

answers.bindings では、配布コードの複数束縛の %let を追い、評価順と回数、前の束縛の参照、後の束縛の先行参照、同名の再束縛、所有値の消費を説明する。pair の分解 let や Wasm の複数戻り値との違いも述べる。
answers.standard では、wlt:std の解決とスコープ、import する名前と別名、Result/Option/Future の型の適用形、引数なしの text/unit の参照、標準関数の選択を説明する。%future.await などの固定構文と、型の参照を区別する。根拠は配布チートシートだけとし、資料不足は明記する。

その後 rubric.md の5軸を0〜4で採点し、各々具体的な行を根拠にする。正しく説明できることを「読みやすい」と評価しない。
改善案は syntax/format/naming/docs に分ける。実行結果や読了時間を推測しない。

JSON形式:
{"answers":{"flow":"...","state":"...","alias":"...","ownership":"...","errors":"...","scope":"...","changes":"...","syntax":"...","bindings":"...","standard":"..."},"scores":{"flow":{"score":0,"evidence":"..."},"types":{"score":0,"evidence":"..."},"scope":{"score":0,"evidence":"..."},"layout":{"score":0,"evidence":"..."},"changes":{"score":0,"evidence":"..."}},"issues":[],"limitations":[]}

issue の形式は {"priority":1,"kind":"syntax|format|naming|docs","location":"...","problem":"...","fix":"..."}。
全軸3以上が目標だが、無理に合格させない。
