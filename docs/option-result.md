# Option / Result の値操作

`wlt:core` の通常の単相化関数として `$option.ok-or` と `$result.map-err` を提供する。新しい制御構文や所有権規則は加えない。

```wat
(module
  (%use "wlt:core" (type $option) (type $result)
    (func $option.ok-or) (func $result.map-err))
  (%closure $Widen (param $error i32) (result i64) (%effect pure)
    (i64.extend_i32_u $error))
  (func $main (export "main") (result i64) (%effect allocates)
    (%match
      (call (%inst $result.map-err i64 i32 i64 $Widen)
        (call (%inst $option.ok-or i64 i32) (%option.some i64 42) 7)
        (%closure.new $Widen))
      (case ok ($value) $value)
      (case err ($error) -1))))
```

- `ok-or<T,E>(option<T>, E)` はsomeをok、noneをerrへ変える。error引数は通常のcallと同じく、someの場合も先に評価される。副作用や割当てを遅延する操作ではない。
- `map-err<T,E,U,F>(result<T,E>, F)` はerrの場合にだけcallbackを一度呼ぶ。okのpayloadをそのまま保ち、GC参照の同一性も維持する。Fは明示的なclosure型で `(param E) (result U)` を満たす。callbackのeffectを具体化した関数のeffectへ反映する。

T/E/Uは共有可能・非unitの値に限定する。所有・借用payloadやunitを扱う場合は、既存の明示的な `%match` を使う。暗黙のmove、借用の延長、数値変換はしない。完全な署名は [標準API](stdlib-api.md)、両JS backendの検査は `just test-value-sugar`。

## 文字列の期待型

文字列型が明示された位置では、`(%string.lit "hello")` を `"hello"` と書ける。対象は関数の引数・結果、`(type $string)`付きlet、フィールド、型を指定したlistなど。透明aliasや具体化済みのgeneric引数も同じ規則を使う。

```wat
(module
  (%use "wlt:core" (type $string))
  (func $id (param $s $string) (result $string) $s)
  (func $main (export "main") (result $string)
    (%let $text (type $string) "日本😀"
      (call $id $text))))
```

`(%let $x "hello" ...)` のように初期値の期待型が未確定の場合は、型注釈か `%string.lit` を書く。UTF-8でないbyte列はstringにしない。stringの長さは `%string.byte-len` でUTF-8のbyte数を得る。`%string.len` という別名や、Unicode文字数への暗黙変換は設けない。
