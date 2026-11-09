# 型引数と単相化

宣言名の直後に `(%type.params $T ...)` を置く。`func`、`type`、`%type.alias`、
`%closure` に共通の文法で、明示的な適用は `(%inst $name T...)`。通常の関数呼び出しでは、
引数・期待型から型引数が一意に決まる場合に省略できる。利用者定義trait・辞書渡し・実行時の型消去は導入しない。通常の WAT の関数と呼び出しはそのまま使える。

```wat
(module
  (func $identity (%type.params $T) (param $x $T) (result $T) (%effect pure)
    (local.get $x))
  (func $main (export "main") (result i64) (%effect pure)
    (call (%inst $identity i64) (i64.const 42))))
```

```wat
(%type.alias $Values (%type.params $T) (%inst $list $T))
(type $Box (%type.params $T) (struct (field $T)))
(%closure $Identity (%type.params $T) (param $x $T) (result $T) (local.get $x))

;; 値型、heap 型、呼び出し先にも同じ適用形を使う。
(param $xs (%inst $Values i64))
(param $box (ref (%inst $Box i64)))
(struct.new (%inst $Box i64) (i64.const 42))
(%closure.call (%inst $Identity i64) (%closure.new (%inst $Identity i64)) (i64.const 42))
```

型引数には数値・値型・GC 参照・具体的な nominal 型名を使える。nominal 名を渡すときは
`(ref $F)`、`closure.call $F` など、その名前を要求する位置で使う。型を要求する位置に
裸の heap 名を置くことはできない。unit も payload の位置に限る既存の規則に従う。
型やクロージャの構築は引数をすべて明示する。未適用の汎用宣言を型や関数の値として渡さない。

同じ宣言と具体的な型引数の組は、モジュール全体で1つの具体化を共有する。
透明な alias は引数の比較前に展開する。nominal 型や異なるクロージャ型の identity は保つ。
同じファイルの import 別名・再公開から使っても具体化は重複しない。
型の位置と nominal 参照だけを置換し、同名のローカル変数・束縛・ラベルを変えない。
opaque な型の具体化にも、元の型を定義した ファイル の制限を適用する。

具体化後に既存の型・effect・所有権・borrow 検査を行い、その後で最適化と DCE を行う。
未使用の通常関数内にある適用も検査する。汎用テンプレートの宣言名・型パラメータ・
参照名は宣言時に確認し、body の型・effect・所有権は各具体化で検査する。
適用されないテンプレートの body に、任意の型引数で正しいという保証は与えない。
検査エラーには body の位置と具体化を要求した位置を付ける。

同じ型引数での再帰関数・再帰 GC 型を扱う。再帰中に型引数を変えて新しい具体化を
要求する polymorphic recursion は拒否する。具体化はキューで処理し、宣言数1024、
依存の深さ64、展開ノード100万を上限とする。推論の型照合にも深さ64・型ノード訪問100万、
具体化を伴う全体再検査には累計200万ノード訪問の上限を設ける。上限超過はコンパイル診断になる。

汎用宣言には直接 Wasm export を付けない。具体的な wrapper を export するか、
`(export "identity" (func (%inst $identity i64)))` と指定する。
WIT と両 JS ABI に出るシグネチャは具体型だけになる。ホスト import にも具体型を要求する。

## 通常の関数呼び出しの推論

```wat
(func $id (%type.params $T) (param $x $T) (result $T) (%effect pure) $x)
(func $answer (result i64) (%effect pure) (call $id 42))
;; 結果の期待型 i64 により T=i64、裸の 42 も i64。
;; (call $id (i32.const 42)) は引数から T=i32。
```

推論の対象は `call $generic` の型引数だけ。関数のシグネチャは宣言し、全域の制約を解かない。
引数・期待する結果型を構造的に照合し、透明な alias、list/option/result/tuple/box 等と nominal な型適用を扱う。
`callable` 制約は既知のクロージャの引数・結果型との等式にも使うので、`(call $list.map xs closure)` から出力要素型も決まる。
その後、明示適用と同じ制約・effect・所有権・race 検査を行う。

型引数が残る場合、型が衝突する場合、`(call $id 1)` のように幅が決まらない場合は `E_GENERIC_INFERENCE`。
`(%inst $id i64)` や型注釈で指定する。数値幅の既定値、数値の暗黙変換、phantom 型引数の推測、
異なる nominal 型をまとめる型の合成は行わない。型とクロージャの適用・export の具体化は引き続き `%inst` を使う。
同じ型の組は明示適用と推論で具体化を共有し、呼び出し箇所ごとの再具体化はしない。

この推論はマクロ内の通常の `call` にも使える。型引数を補うためだけのマクロは不要で、名前の束縛や構文の組み替えが必要な場合に [ユーザー定義マクロ](user-macros.md) を使う。`infer` の公開 selector は要素型などを読み取るためのもので、新しい型規則は登録しない。

通常のマクロ展開では影響する式から検査を再開するが、未生成の具体化を必要とする場合は型環境を作り直して全体を検査する。推論中に作った一時ローカルや、以前のリビジョンの検査済み型を流用しない。型・effect・所有権の契約は具体化後にも検査する。

## 具体的なクロージャ型を渡す map / fold

`stdlib/list.wlt` は named import で次の汎用関数を公開する。

```wat
(%use "wlt:core" (func $list.map) (func $list.fold))
(call (%inst $list.map i64 $string $Label) xs label-closure)
(call (%inst $list.fold i64 i64 $Sum) xs sum-closure initial)
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

## 具体化時の型引数制約

`%type.params` の直後に、任意の `(%where constraint...)` を1つ置ける。
func/type/type.alias/closure に共通で、制約はすべて満たす必要がある。
制約の対象は、その宣言の型パラメータを明示する。

```wat
(func $map-pure (%type.params $T $U $F)
  (%where
    (shared $T $U)
    (nonunit $T $U)
    (callable $F (param $T) (result $U) (%effect pure)))
  (param $xs (%inst $list $T)) (param $f (ref $F)) (result (%inst $list $U))
  (call (%inst $list.map $T $U $F) (local.get $xs) (local.get $f)))
```

| 制約 | 判定 |
| --- | --- |
| `(shared $T...)` | 所有値と借用を含まない共有可能な型。GC 型・値型・クロージャの捕捉も再帰的に調べる |
| `(nonunit $T...)` | unit 以外。格納や引数としての型の正当性は通常の型検査でも確認する |
| `(Add $T...)` / `(Eq $T...)` / `(Ord $T...)` | [組み込み型クラス](typeclasses.md)の固定インスタンスがある具体型 |
| `(Sub $T...)` / `(Mul $T...)` / `(Div $T...)` | 加算と同じ4つのスカラー型。整数除算はWasmの符号付き除算 |
| `(Logical $T...)` / `(Bitwise $T...)` / `(Shift $T...)` | 論理演算はi32、ビット演算・シフトはi32/i64。論理演算は短絡するifへ展開 |
| `(callable $F (param T...) (result U) [(%effect ...)])` | F が共有・再呼出し可能な具体的な nominal クロージャ型で、引数列と戻り値の型が一致する |

shared は不変性を要求しない。可変 GC 配列・セルへの参照も、その中身が共有可能なら許可する。
callable の署名は alias を展開した具体型で厳密に比較し、引数の反変性等の規則は追加しない。
`(param)` は引数なし、`(result $unit)` は unit 戻り値。所有する捕捉を持つ一度きりの
クロージャは callable を満たさない。引数や戻り値に所有型を明示すること自体は許可する。

`%effect` を省略した callable は effect の上限を課さない。指定する場合は通常の effect 契約と同じ
reads/writes/allocates/suspends/host、`(throws $Tag...)`、`(throws *)` を使う。
pure は単独で指定する。再帰・呼び出し先・catch を含む実際の本体から推論し、
関数自身の effect 宣言を証拠として扱わない。ホスト import は信頼する policy に従い、
未指定なら未知の effect として保守的に扱う。

型と callable の署名は具体化した宣言のヘッダーがそろった時点で、本体の型検査より前に検査する。
effect の上限は本体を型検査し、ホスト policy を適用した後に検査する。
どちらも DCE・最適化より前に行い、呼び出されない callback や未使用の通常関数内の適用も検査する。
適用・制約の両方の位置を `E_GENERIC_BOUND` で報告し、effect 違反には呼び出し経路を付ける。

未適用でも where の位置・構造・対象パラメータ名・effect の文法は検査する。
抽象的な型変数を使った body の宣言時検査、利用者定義の型クラス・実装探索・辞書渡しは追加しない。
制約は検査用の情報として消去し、実行用の関数やランタイムの状態を増やさない。
stdlib/list.wlt の map/fold も shared/nonunit/callable を宣言する。
例の `scale-pure` は純粋なコールバックに限定した map を両 ABI へ公開する。
