# bytes / string / array のゼロコピービュー

`$bytes-view` と `$string-view` は、不変なデータの一部を共有する標準ライブラリの不透明なGC型。
`wlt:core` から必要な型・関数を明示的に import する。新しい構文や実行時の管理テーブルは追加しない。
標準の文字列型は `$string`。`$` 付きの名前なので、WAT のキーワード `string` とは別の名前になる。
基底型と命令は既存の `%string` / `%string.*` を使い、内容は厳密なUTF-8。

```wat
(module
  (%use "wlt:core"
    (type $string)
    (func $string-view.new) (func $string-view.slice)
    (func $string-view.to-string))
  (func $crab (export "crab") (param $source $string)
    (result $string) (%effect allocates)
    (%let (($whole (call $string-view.new $source))
           ($part (call $string-view.slice $whole 1 7))
           ($crab (call $string-view.slice $part 3 4)))
      ;; "x日🦀z" なら "🦀"。コピーはここで行う。
      (call $string-view.to-string $crab))))
```

ビューを使って読む例は [examples/views.wlt](../examples/views.wlt)。
正確な署名は [標準API一覧](stdlib-api.md) にある。

| 関数 | 操作 | 計算量 |
| --- | --- | --- |
| `$bytes-view.new(bytes)` | 全体のビューを作る | O(1) |
| `$bytes-view.len(view)` | バイト長 | O(1) |
| `$bytes-view.get(view,index)` | ビュー内のbyteを0–255のi32として読む | O(1) |
| `$bytes-view.slice(view,start,length)` | ビュー先頭からの相対範囲を共有する | O(1) |
| `$bytes-view.to-bytes(view)` | 表示範囲を独立したbytesへコピーする | O(n) |
| `$string-view.new(string)` | 文字列全体のビューを作る | O(1) |
| `$string-view.byte-len(view)` | UTF-8のバイト長 | O(1) |
| `$string-view.bytes(view)` | 同じ範囲のbytesビューを返す | O(1) |
| `$string-view.slice(view,start,length)` | UTF-8境界を検査し、部分ビューを作る | O(1) |
| `$string-view.to-string(view)` | 表示範囲を独立した文字列へコピーする | O(n) |

`new` と `slice` は小さなGCオブジェクトを確保するため `allocates` を持つ。
残りは semantic effect が `pure`。`pure` は無trapや物理的なGC割当てなしを意味しない。
「ゼロコピー」は、ビュー生成・再スライス・参照でデータ本体をコピーしないという契約。

## 範囲とUTF-8

添字と長さは既存のbytes操作と同じi32の符号なしビットパターンで、範囲は `start` と `length`。
`start <= view.length` と `length <= view.length - start` を順に検査し、加算の桁あふれを避ける。
範囲外の `slice` と `get` はWasmのtrap。クランプや暗黙の丸めはしない。
元のバッファ内でもビューの外側を読む操作は拒否する。

stringビューは開始位置と終端がUTF-8の文字境界であることも検査する。
たとえば `"é"` の全体は `(start=0,length=2)`。`(0,1)` や `(1,0)` は文字の途中なので拒否する。
`(2,0)` は有効な空ビュー。文字数・書記素数・正規化は扱わない。
stringビューから得たbytesビューは、任意のbyte範囲へさらに分割できる。

## 共有と回収

bytesビューは元のbytesへの参照・開始位置・長さを保持する。
stringビューはUTF-8の不変条件を満たすbytesビューを保持する。
再スライスの結果は元のデータを直接参照し、親ビューを連鎖して保持しない。
GCが参照を追うため、呼出し元を抜けた後やfutureを待っている間もデータは有効。
可変ビュー、borrow lifetime、明示的な解放は追加しない。

小さなビューも元のバッファ全体を保持する。
長期間保存する一部だけを切り離すには `to-bytes` / `to-string` を使う。全体ビューでも内容をコピーする。
ビューが到達不能になれば元データへの参照も失われ、コピーした値は独立して生存できる。

ビューはゲスト内のGC参照で、WITの値型として直接exportできない。
Core＋JSPI / Componentの公開境界ではbytes/stringへ明示的に変換し、既存のABI adapterへ渡す。
ABI越しのゼロコピーはこのAPIの契約に含めない。

## 任意のGC配列を共有するarray-view

`$array-view`は読み取り専用のジェネリックなビュー。要素型`$T`と、元の名前付き配列型`$A`を明示する。
言語の新しい構文やarray用の型クラスは追加せず、既存のジェネリクスと明示captureのクロージャで実装する。

```wat
(module
  (%use "wlt:core"
    (func $array-view.new) (func $array-view.slice) (func $array-view.get))
  (type $Numbers (array (mut i64)))
  (func $main (export "main") (result i64) (%effect reads writes allocates)
    (%let (($source (array.new_fixed $Numbers 4 99 40 0 99))
           ($whole (call (%inst $array-view.new i64 $Numbers) $source))
           ($part (call (%inst $array-view.slice i64 $Numbers) $whole 1 2)))
      (block (result i64)
        (array.set $Numbers $source 2 2)
        (+ (call (%inst $array-view.get i64 $Numbers) $part 0)
           (call (%inst $array-view.get i64 $Numbers) $part 1))))))
```

このmainは42を返す。元配列の変更を、作成済みのビューから観測できる。
ビューからの書き込み操作や、不変なスナップショットという保証は追加しない。

型は`(ref (%inst $array-view T $A))`。`T`はsharedかつnonunit、`$A`はsharedな名前付きGC配列型で、
その要素型が`T`に一致する必要がある。可変／不変どちらの配列も扱える。
型付きコアが扱う非packed配列が対象で、数値・v128・共有GC参照・stringなどの要素を使える。
getterを呼ぶ前でも、newの具体化時に配列の種類と要素型を検査する。

すべての関数を`(call (%inst $array-view.<操作> T $A) ...)`で呼ぶ。

| 操作 | 引数 → 結果 | effect | 計算量 |
| --- | --- | --- | --- |
| `new` | 元配列 → view | allocates | O(1) |
| `len` | view → i32 | pure | O(1) |
| `get` | view, index → T | reads | O(1) |
| `slice` | view, start, length → view | allocates | O(1) |
| `to-list` | view → `(%inst $list T)` | reads writes allocates | O(n) |

範囲の単位は要素数で、添字はビュー先頭から相対的に解釈する。
bytes-viewと同じ符号なしi32の範囲検査を行い、範囲外・負のビットパターン・桁あふれする範囲はtrapする。
末尾の長さ0のsliceは有効。空ビューのgetはtrapし、to-listは空listを返す。

newは元配列を捕捉した小さなreaderとviewを確保し、sliceは同じreaderを共有したviewだけを確保する。
親ビューを連鎖して保持せず、要素領域はコピーしない。元配列はビューを通じてGCに追跡され、中断中も生存する。
getは可変配列を読む可能性があるので、bytes/stringビューと異なり保守的にreadsを持つ。

to-listは表示範囲を作業配列に読み、その範囲の不変listを作る。空でない場合は作業配列とlistの格納領域を確保する。
元配列やreaderへの参照はlistに格納しない。参照型の要素自体は共有するため、要素グラフまで深くコピーする操作ではない。
他に参照がなく、要素からの参照もなければ、ビューを捨てた後に元配列を回収できる。
WIT／JS ABIの境界では、公開可能な要素型に限りto-listで明示的に値へ変換する。
v128や生のGC参照はlistの要素にしてもこのABIでは公開できない。GC参照であるビューそのものも境界に渡さない。

## 検査と参考

`just test-views` は範囲・Unicode・opaque/effect検査、Core/ComponentのABI、待機中のGC保持を調べる。
array-viewは可変配列の共有、型・制約、非default参照とSIMD要素、空のコピー、範囲外のtrapも検査する。
生成IRの到達可能な経路にデータコピーがないことを検査し、テスト専用のGC観測器でも
ストレージの共有、親ビューの回収、コピー後の元バッファの回収を確かめる。
Chromiumの検査は `just test-e2e` に含まれる。

参照と範囲を分ける設計は、MoonBitの [BytesView](https://github.com/moonbitlang/core/blob/main/builtin/bytesview.mbt)
と [StringView](https://github.com/moonbitlang/core/blob/main/builtin/stringview.mbt) を参考にした。
MoonBitのStringViewはUTF-16のコード単位、WLTのstringビューはUTF-8のバイト単位で境界を検査する。
