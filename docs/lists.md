# 不変の list<T>

`(list T)` は要素型ごとに具体化する組込みの値型。コンテナを変更する操作はなく、
`bytes` と同様に内部の GC 配列を参照・比較・変更することはできない。
要素には共有の数値・値型・GC 参照を使える。所有値と borrow は格納できない。
GC 参照の要素が指すオブジェクトの可変性は、その参照型の規則に従う。

| 構文 | 結果・性質 |
| --- | --- |
| `(list.new T value...)` | 左から一度ずつ評価して作る。引数なしなら空 |
| `(list.len T xs)` | i32 の要素数 |
| `(list.get T xs index)` | 要素。添字は符号なしで扱い、範囲外は Wasm trap |
| `(list.slice T xs start length)` | 範囲をコピー。範囲外は trap。終端の長さ0は有効 |
| `(list.from-array T $Array source)` | 要素型が同じ GC 配列からコピーする |

slice/from-array はコンテナをコピーする。GC 参照の要素は参照を共有する。
元の可変配列を書き換えても、コピー済み list の要素スロットは変わらない。
`list.new unit` には要素数に応じた `(nop)` などの unit 式を書く。

```wat
(list.get i64
  (list.new i64 (i64.const 40) (i64.const 2))
  (i32.const 1))
```

可変 GC 配列で結果を構築してから list にコピーする map と、list を読む sum の例を
`examples/list-batch.wlt` に置く。汎用の map/fold は [単相化](generics.md) と
`examples/generic-lists.wlt` を参照。

WIT は `list<T>`、Canonical ABI は要素領域の ptr/length を使う。
要素の size/alignment に従い、入れ子の領域も再帰的に解放する。読み込みは長さ・乗算・
アラインメント・メモリ範囲を検査する。ABI の線形メモリ上限は既存の16 MiB。
Wasm GC 配列の到達性はエンジンに任せ、独自の GC ランタイムは加えない。

JS には配列、数値要素には対応する TypedArray も渡せる。
数値 list の返り値は Int32Array/BigInt64Array/Float32Array/Float64Array、それ以外は Array。
これは Component の生成 binding に合わせた形。s64 の値は BigInt を使う。
WIT `list<u8>` は引き続き `bytes` / Uint8Array へ対応する。
List を Future/Stream の共有 payload にすることはできるが、Future/Stream を list に格納することはできない。

検査は `just test-lists`。Rust の型・所有権・escape 解析、Wasmtime/Node の副作用順、
両 JS backend の往復・部分変換失敗時の回収、Chromium の境界動作を確認する。
所有権の契約言語は変更せず、list の引数を既存の Seq として検査する。

変換失敗や trap では、guest に登録した endpoint を閉じてから ABI arena を回収する。
固定した jco 0.23.0 の失敗した copy の状態も終了させ、待機中の Stream を失敗で起こす。
管理表の残数と iterator.return、未処理の Promise が発生しないことを異常系で検査する。
