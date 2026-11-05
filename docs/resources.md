# 外部資源の最小契約

```wat
(type $File (resource "file" $close))
(using $file (resource.new $File (call $open))
  (call $read-file (local.get $file)))
```

`(resource $File)` は所有値、`(borrow $File)` は `using` または関数引数でだけ扱う借用。
`resource.close` は所有値を消費し、登録した destructor を一度呼ぶ。
`using` は所有値を消費し、正常値・Result.err・Wasm 例外・取消しを通して終了時に close する。
ループは内側で使えるが、外側への `recur` は close を飛ばすため拒否する。

borrow は返却・GC 格納・クロージャ捕捉・例外 payload にできない。
`resource.handle` は、その kind の host 操作の第1引数にだけ書ける。
一般の lifetime、借用した所有値の返却、async destructor は追加しない。
所有値を持つ関数の try/catch と、例外を投げうる関数呼出しは引き続き拒否する。
明示的な throw は、残った所有値がゼロであることを Stop 契約で検査する。
catch は所有値を持たない外側の関数に書ける。

ホストの policy に `resource: {kind, operation}` を加える。operation は new/use/close。
new は同期 factory の値を返す i32 handle、use は第1引数の handle をホスト値へ変換する。
close は同期の i32→unit。ホスト実装には数値 handle ではなく元の資源オブジェクトが渡る。
kind ごとに destructor を一つ指定する。use から所有 endpoint を返すことは初版では認めない。

ホスト registry は task と kind を検査し、閉じた ID を再利用しない。
Wasm trap は catch できないため、task 終了時の registry の解放を併用する。
解放は逆順で、ひとつ失敗しても残りを試みる。取消しの完了通知は解放を待つ。
destructor 失敗後の instance は再利用しない。

WIT native resource の own/borrow ABI は後続。現在は明示的な host adapter であり、
WASI の resource ABI と同一ではない。数値の低水準 import はライブラリの非公開部分に置く。
`examples/resources/` と Node/Chromium の検査に、実ファイルと全終了経路の例がある。
Lean が検査するのは抽出した所有権契約。borrow の非流出、Wasm finally、host registry は TCB。
