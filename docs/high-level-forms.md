# リスト走査・構造体分解・フィールド更新

3つとも型付きコアの糖衣。`%for-each` は `let`・リスト操作・`loop/if/block/recur`、構造体の pattern let は `let/struct.get`、`%struct.update` は `let/struct.get/struct.set` に展開する。追加のランタイムや汎用 iterator は持たない。型・effect・所有権・opaque の検査を通した後で最適化する。

実行例は [examples/high-level.wlt](../examples/high-level.wlt)。`wlt run examples/high-level.wlt` は `42` を返す。検証は `just test-high-level`。

## `%for-each`

```wat
(%for-each $item $items
  (call $visit $item))

(%for-each ($key $amount) $entries
  (call $add $key $amount))

(%for-each {:category $key :amount $amount} $rows
  (call $add $key $amount))
```

形式は `(%for-each pattern list-expression unit-body)`。リスト式と長さを一度だけ評価し、添字0から順番に各要素を一度ずつ読む。空なら本体を評価しない。本体・全体の型は unit。要素型はリストの型から決める。要素は共有可能な非unit値とし、所有値・borrow を含む要素は受け付けない。

pattern は `$name`、既存の tuple の `($first $second)`、後述の構造体フィールド pattern。tuple の unit payload は既存の分解規約どおり束縛を持たない。各反復で新しいローカル ID に束縛し、外側の同名ローカルを隠せる。リスト式の評価中にはその新しい束縛は見えない。

本体は通常の effect・所有権検査を受ける。空のリストに対しても本体の型や effect を検査する。反復ごとに作って消費する所有値は使えるが、外側の所有値を反復ごとに消費することはできない。

内部の添字・長さ・ループラベルはソースから参照できない。専用の `break/continue` やラベル引数は設けない。全体からの早期脱出は外側の `block` と標準の `br`、または `return` を使う。例外や脱出が起きれば以後の要素を評価しない。内部ループをまたぐ外側への `%recur` は拒否する。

```wat
(block $done (result i32)
  (%for-each $n $numbers
    (if (< $n 0)
      (then (br $done $n))
      (else (nop))))
  0)
```

配列・view・stream は直接の対象にしない。配列の可変性や stream の所有権を走査構文へ暗黙に混ぜず、既存の操作で明示する。

## 構造体の pattern let

```wat
(%let {:category $key :amount $amount} $row
  (call $add $key $amount))
```

形式は `(%let {:field $binding ...} struct-expression body)`。対象は型が確定した non-null の共有構造体参照。対象式を一度だけ評価し、指定したフィールドを記述順に一度ずつ読んでから本体を評価する。必要なフィールドだけ選べる。フィールドの重複、束縛名の重複、未知のフィールド、非識別子の束縛はエラー。空の pattern は受け付けない。

束縛の型はフィールドの型そのもの。型注釈・入れ子の pattern・デフォルト値は追加しない。この形式は単一の `%let` で使い、複数初期化のグループには混在させない。グループの内側にこの `%let` を置ける。

可変フィールドも読み取った時点の値を束縛する。その後に元のフィールドへ代入してもローカルの値は変わらない。GC参照の場合は参照先を共有し、オブジェクトを複製しない。可変フィールドの読み取りは `reads`、不変フィールドは既存の `struct.get` と同じ effect を持つ。opaque 型の内部フィールドは宣言元以外で分解できない。

## `%struct.update`

```wat
(%struct.update $State $state
  ($accepted $old (+ $old 1)))
```

形式は `(%struct.update $Type object-expression ($field $old new-value))`。一つの式で一つのフィールドを更新し、unit を返す。フィールドは `struct.set` と同様に名前または添字で指定できる。型オペランドは `(%inst $Type T...)` も使える。

評価順序は次のとおり。

1. 対象式を一度だけ評価する。
2. そのフィールドを一度読み、`$old` に束縛する。null ならここで trap し、更新式には進まない。
3. 更新式を一度評価する。`$old` はこの式の中だけで見える。
4. 得られた値を同じ対象・同じフィールドへ一度書く。

対象は共有構造体、フィールドは可変で共有可能な非unit型とする。更新式にはそのフィールド型を期待する。暗黙の型変換・move・部分moveは行わない。不変フィールドや外部の opaque 型への更新は拒否する。通常の `reads writes` を持ち、更新式・対象式の追加 effect も検査する。

更新式には通常の関数呼び出し・例外・中断を使える。旧値は更新式に入る前に確定する。同じフィールドを更新式内で別途変更しても、正常終了すれば最後の書き込みで上書きする。例外・`return`・外側への `br` で更新式が終わった場合、最後の書き込みは行わず、それまでの副作用は残る。原子的な更新・排他・ロールバックは提供しない。共有線形メモリの競合には [atomic cells](atomic-cells.md) を使う。

Core、Core＋JSPI、Component は同じ検査済みIRを使う。中断中のローカル保持、例外時の中止、ABI一時メモリの解放は Node / Chromium の両バックエンド検査で確認する。糖衣の意味保存を新たにLeanで証明したものではなく、既存の所有契約の検査をそのまま使う。
