# 外部資源の最小契約

```wat
(type $File (%resource "file" $close))
(%using $file (%resource.new $File (call $open))
  (call $read-file (local.get $file)))
```

`(%inst $resource-handle $File)` は所有値、`(%inst $borrow $File)` は `%using` または関数引数でだけ扱う借用。
`%resource.close` は所有値を消費し、登録した destructor を一度呼ぶ。
`%using` は所有値を消費し、正常値・Result.err・Wasm 例外・取消しを通して終了時に close する。
return・br・br_if・外側のループへの `%recur` でも、抜ける using を逆順に close する。分岐しない br_if と、using の内側だけを抜ける分岐は資源を早く閉じない。

borrow は返却・GC 格納・クロージャ捕捉・例外 payload にできない。
`%resource.handle` は、その kind の host 操作の第1引数にだけ書ける。
一般の lifetime、借用した所有値の返却、async destructor は追加しない。
所有値と try/catch・例外を投げる呼出しを併用できる。通常・例外・分岐先の所有集合を検査し、失われる所有値と、後続の引数評価が例外を投げて取り残される所有一時値を拒否する。捕捉されない throw では未消費の所有値を残せない。[詳細](core-expressions.md) を参照。borrow は分岐の payload にもできない。

ホストの policy に `resource: {kind, operation}` を加える。operation は new/use/reader/close。
new は同期 factory の値を返す i32 handle、use は第1引数の handle をホスト値へ変換する。
close は同期の i32→unit。ホスト実装には数値 handle ではなく元の資源オブジェクトが渡る。
kind ごとに destructor を一つ指定する。use は所有値を返せない。同期の reader 契約だけが、借用と独立した Future / Stream を生成できる。reader の単一取得はホストの protocol が検査する。
using はこの独立した Future / Stream と、借用を捕捉できない Task の返却を許す。borrow やその他の所有値を返すことは禁止する。[writer と子タスク](scoped-tasks.md)。

ホスト registry は task と kind を検査し、閉じた ID を再利用しない。
Wasm trap は catch できないため、task 終了時の registry の解放を併用する。
解放は逆順で、ひとつ失敗しても残りを試みる。取消しの完了通知は解放を待つ。host Promise が abort で reject する場合も、disposer の完了後に guest を unwind して close する。
destructor 失敗後の instance は再利用しない。

WIT native resource の own/borrow ABI は後続。現在は明示的な host adapter であり、
WASI の resource ABI と同一ではない。数値の低水準 import はライブラリの非公開部分に置く。
`examples/resources/` と Node/Chromium の検査に、実ファイルと全終了経路の例がある。
Lean が検査するのは抽出した所有権契約。borrow の非流出、Wasm finally、host registry は TCB。
