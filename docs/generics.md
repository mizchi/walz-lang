# 明示的な型引数と単相化

宣言名の直後に `(type.params $T ...)` を置く。`func`、`type`、`type.alias`、
`closure` に共通の文法で、適用は `($name T...)`。型引数の推論・trait・辞書渡し・
実行時の型消去は導入しない。通常の WAT の関数と呼び出しはそのまま使える。

```wat
(module
  (func $identity (type.params $T) (param $x $T) (result $T) (effects pure)
    (local.get $x))
  (func $main (export "main") (result i64) (effects pure)
    (call ($identity i64) (i64.const 42))))
```

```wat
(type.alias $Values (type.params $T) (list $T))
(type $Box (type.params $T) (struct (field $T)))
(closure $Identity (type.params $T) (param $x $T) (result $T) (local.get $x))

;; 値型、heap 型、呼び出し先にも同じ適用形を使う。
(param $xs ($Values i64))
(param $box (ref ($Box i64)))
(struct.new ($Box i64) (i64.const 42))
(closure.call ($Identity i64) (closure.new ($Identity i64)) (i64.const 42))
```

型引数には数値・値型・GC 参照・具体的な nominal 型名を使える。nominal 名を渡すときは
`(ref $F)`、`closure.call $F` など、その名前を要求する位置で使う。型を要求する位置に
裸の heap 名を置くことはできない。unit も payload の位置に限る既存の規則に従う。
型引数の個数は常に明示し、未適用の汎用宣言を型や関数の値として渡さない。

同じ宣言と具体的な型引数の組は、モジュール全体で1つの具体化を共有する。
透明な alias は引数の比較前に展開する。nominal 型や異なるクロージャ型の identity は保つ。
同じファイルの import 別名・再公開から使っても具体化は重複しない。
型の位置と nominal 参照だけを置換し、同名のローカル変数・束縛・ラベルを変えない。
opaque な型の具体化にも、元の型を定義した namespace の制限を適用する。

具体化後に既存の型・effect・所有権・borrow 検査を行い、その後で最適化と DCE を行う。
未使用の通常関数内にある適用も検査する。汎用テンプレートの宣言名・型パラメータ・
参照名は宣言時に確認し、body の型・effect・所有権は各具体化で検査する。
適用されないテンプレートの body に、任意の型引数で正しいという保証は与えない。
検査エラーには body の位置と具体化を要求した位置を付ける。

同じ型引数での再帰関数・再帰 GC 型を扱う。再帰中に型引数を変えて新しい具体化を
要求する polymorphic recursion は拒否する。具体化はキューで処理し、宣言数1024、
依存の深さ64、展開ノード100万を上限とする。上限超過はコンパイル診断になる。

汎用宣言には直接 Wasm export を付けない。具体的な wrapper を export するか、
`(export "identity" (func ($identity i64)))` と指定する。
WIT と両 JS ABI に出るシグネチャは具体型だけになる。ホスト import にも具体型を要求する。

## 具体的なクロージャ型を渡す map / fold

`stdlib/list.wlt` は named import で次の汎用関数を公開する。

```wat
(import "../stdlib/list.wlt" (func $list.map) (func $list.fold))
(call ($list.map i64 text $Label) xs label-closure)
(call ($list.fold i64 i64 $Sum) xs sum-closure initial)
```

map は共有・非 unit の T/U、fold は共有・非 unit の T/S と、繰り返し呼べる具体的な
クロージャ型 F を使う。異なる型や契約を渡せば具体化の検査で拒否する。
空の map はコールバックを呼ばず、それ以外は先頭から一度ずつ呼ぶ。
空の fold は初期値を返す。GC 配列は map 内部でだけ変更し、返す list にはコピーする。
間接呼び出しやコールバック用の新たなランタイムは追加しない。
コールバックの effect も具体化後の閉じた呼び出しグラフから求める。

実行できる例は [generic-lists.wlt](../examples/generic-lists.wlt)。
`just test-generics` で、最適化の有無、Wasmtime、両 JS ABI、メモリ回収、
具体化の共有・所有権・opaque・名前の衝突・展開上限を検査する。
