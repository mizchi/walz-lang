# 型付きマクロと WAT 命令

標準 WAT のモジュールは従来通り Wasm validator で検査する。型付き WLT の関数内でも、数値・SIMD の線形メモリ命令を `%let`、制御、GC と混ぜて使える。

```wat
(module
  (memory $data 1)
  (func $main (export "main") (result i32) (%effect reads writes)
    (%let $pointer (type i32) 16
      (block (result i32)
        (i32.store $data $pointer (i32.const 42))
        (i32.load $data $pointer)))))
```

scalar/SIMD の load/store、splat・lane load/store、`memory.copy` / `memory.fill` を扱う。添字は memory32 の i32。ファイルモジュールでは明示した memory 名を名前解決し、DCE も依存を保持する。JS ABI のメモリとは別であり、出力も常にメモリ名を付ける。

入力型と結果型は、小さい検査用モジュールを作り `wasmparser::FuncValidator` の operand stack と照合する。alignment、lane、offset とアドレス・値の型も Wasm の規則を使う。命令の署名を独自に再定義しない。

effect の分類は明示した命令の閉じた一覧。load は reads、store/fill は writes、copy は両方、物理的には trap し得る。解析では不明な線形メモリの読み書きとして扱い、純粋・非別名と仮定しない。`%race-free` では raw access を拒否する。

命令に渡す値は検査済みの数値。owned / borrow / GC ref を不透明な命令に渡せず、newtype は明示的に unwrap する。数値を返す checked expression の内部では、従来の所有権と effect 検査が働く。

任意の stack 区間、table/call/ref 命令の一般的な opaque escape、多値結果、memory64、passive data はこの橋渡しの対象外。型付き拡張の未知の命令は拒否する。通常 WAT だけのモジュールに対する受付範囲を狭めるものではない。
