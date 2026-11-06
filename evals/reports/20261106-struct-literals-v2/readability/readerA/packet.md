# structの構築を読む

この資料だけを読む。構文の出自・既存/新規・ユーザーの好みを推測して優劣を付けない。各表記を独立に採点し、最後に用途別の推薦を述べる。全て同じ固定フィールドのGC structを表す。汎用mapやJSON値ではない。全fieldが必要で、重複・余剰・欠落を拒否する。mutableの意味は宣言に従う。暗黙のdefault、spread、keyword値はない。%letの複数束縛は上から順に初期値を各1回評価して束縛し、後の初期値とbodyから前の束縛を参照できる。全ての式は左から右に評価する。

以下の宣言と束縛の文脈を利用できる。$markは番号をtraceへ記録してその番号を返す。各caseは独立。

```wat
(%import "wlt:std" (type $text) (type $bytes))
(type $User (struct (field $name $text) (field $age i32) (field $email $text)))
(type $Bucket (struct (field $count (mut i64)) (field $total (mut i64)) (field $max (mut i64))))
(type $State (struct (field $accepted i64) (field $rejected i64) (field $ignored i64)
  (field $info (ref $Bucket)) (field $warn (ref $Bucket)) (field $error (ref $Bucket))))
(type $Pair (struct (field $left i32) (field $right i32)))
(type $Cell (struct (field $value (mut i32))))
(type $Link (struct (field $next (ref null $Link))))
(type $Job (struct (field $id $text) (field $key $bytes) (field $index i32)
  (field $duration i64) (field $deps (mut (ref null $Link)))))
(type $Trace (struct (field $log (mut i32))))
```

# struct構築の可読性 v1

各軸0–4。点数は初見のLLMによる主観評価で、人間の読了時間や誤答率の測定ではない。
正しく答えられたことだけで可読性を加点しない。各軸の根拠をケースIDと構文の箇所で示す。

| 軸 | 4 | 3 | 2 | 1 | 0 |
|---|---|---|---|---|---|
| fields | 複数・同型・nestedの全フィールドの対応が局所的に分かる | 一部だけ定義の確認が必要 | 複数箇所で定義の順序と照合が必要 | 常に順番を数え直す | 対応が分からない |
| types | 値型・nominal型・mutableを局所的に判別できる | 小さな宣言確認で判別できる | 期待型や記法の規則を複数回確認する | 型やmapの意味を推測する | 意味を取り違える |
| order | 記述順とlayout順を混同せず局所的に追える | 1つの規則・束縛の確認で追える | 補助束縛と規則を行き来する | 順序を推測する | 順序が判別できない |
| layout | 小型・大型・nestedとも値の境界が明確 | 一部の記号や括弧の確認が必要 | 記号の反復や閉じ括弧を複数箇所で追う | 式の区切りを常に数える | 式の境界を判別できない |
| changes | 同型fieldの変更箇所が名前から直ちに分かる | 1回の定義照合で変更できる | 複数回の定義照合・数え直しが必要 | 他fieldの変更を混入しやすい | 指定fieldを変更できない |

合計点で弱い軸を相殺しない。仕様に書かれていない慣習を勝手に補った場合はunclearへ記録する。
推薦は用途別でもよい。文字数の少なさと読みやすさ、表記の好みと正読を区別する。


## 表記R

(struct.new $T e1 e2 ...) は型定義のフィールド順に初期化する。値は左から右に各1回評価する。

### R.user

name, age, emailの値を答える。

```text
(struct.new $User
  (%text.lit "Alice")
  (i32.const 30)
  (%text.lit "alice@example.com")
)
```

### R.bucket

count, total, maxの値とtotalType（i32/i64）を答える。

```text
(struct.new $Bucket
  (i64.const 7)
  (i64.const 91)
  (i64.const 22)
)
```

### R.nested

accepted, warnTotal（warn.total）, errorMax（error.max）を答える。

```text
(struct.new $State
  (i64.const 11)
  (i64.const 2)
  (i64.const 3)
  (struct.new $Bucket
    (i64.const 10)
    (i64.const 100)
    (i64.const 17)
  )
  (struct.new $Bucket
    (i64.const 4)
    (i64.const 19)
    (i64.const 8)
  )
  (struct.new $Bucket
    (i64.const 1)
    (i64.const 5)
    (i64.const 5)
  )
)
```

### R.order

trace（呼出し番号の配列）, left, rightを答える。$markは引数の番号を$traceへ記録し、同じ番号を返す。

```text
(%let (($right (call $mark (local.get $trace) (i32.const 2)))
  ($left (call $mark (local.get $trace) (i32.const 1))))
  (struct.new $Pair (local.get $left) (local.get $right)))
```

### R.cell

valueとmutable（構築後にstruct.setできるか、boolean）を答える。

```text
(struct.new $Cell
  (i32.const 40)
)
```

### R.job

index, durationType（i32/i64）, depsNull（boolean）, keyFrom（id/index/duration）を答える。$id:$text="task-7", $index:i32=3, $duration:i64=40という束縛を利用できる。

```text
(struct.new $Job
  (local.get $id)
  (%text.bytes (local.get $id))
  (local.get $index)
  (local.get $duration)
  (ref.null $Link)
)
```

## 表記S

(%struct.new $T ($field expression) ...) はフィールド名で初期化する。記述順は任意。値は記述順に各1回評価する。

### S.user

name, age, emailの値を答える。

```text
(%struct.new $User
  ($name (%text.lit "Alice"))
  ($age (i32.const 30))
  ($email (%text.lit "alice@example.com"))
)
```

### S.bucket

count, total, maxの値とtotalType（i32/i64）を答える。

```text
(%struct.new $Bucket
  ($count (i64.const 7))
  ($total (i64.const 91))
  ($max (i64.const 22))
)
```

### S.nested

accepted, warnTotal（warn.total）, errorMax（error.max）を答える。

```text
(%struct.new $State
  ($accepted (i64.const 11))
  ($rejected (i64.const 2))
  ($ignored (i64.const 3))
  ($info (%struct.new $Bucket
    ($count (i64.const 10))
    ($total (i64.const 100))
    ($max (i64.const 17))
  ))
  ($warn (%struct.new $Bucket
    ($count (i64.const 4))
    ($total (i64.const 19))
    ($max (i64.const 8))
  ))
  ($error (%struct.new $Bucket
    ($count (i64.const 1))
    ($total (i64.const 5))
    ($max (i64.const 5))
  ))
)
```

### S.order

trace（呼出し番号の配列）, left, rightを答える。$markは引数の番号を$traceへ記録し、同じ番号を返す。

```text
(%struct.new $Pair
  ($right (call $mark (local.get $trace) (i32.const 2)))
  ($left (call $mark (local.get $trace) (i32.const 1)))
)
```

### S.cell

valueとmutable（構築後にstruct.setできるか、boolean）を答える。

```text
(%struct.new $Cell
  ($value (i32.const 40))
)
```

### S.job

index, durationType（i32/i64）, depsNull（boolean）, keyFrom（id/index/duration）を答える。$id:$text="task-7", $index:i32=3, $duration:i64=40という束縛を利用できる。

```text
(%struct.new $Job
  ($id (local.get $id))
  ($key (%text.bytes (local.get $id)))
  ($index (local.get $index))
  ($duration (local.get $duration))
  ($deps (ref.null $Link))
)
```

## 表記T

(%T::{:field expression ...}) は宣言された$Tをフィールド名で初期化する。記述順は任意。値は記述順に各1回評価する。値の式はWAT/WLTの通常の式。

### T.user

name, age, emailの値を答える。

```text
(%User::{
  :name (%text.lit "Alice")
  :age (i32.const 30)
  :email (%text.lit "alice@example.com")
})
```

### T.bucket

count, total, maxの値とtotalType（i32/i64）を答える。

```text
(%Bucket::{
  :count (i64.const 7)
  :total (i64.const 91)
  :max (i64.const 22)
})
```

### T.nested

accepted, warnTotal（warn.total）, errorMax（error.max）を答える。

```text
(%State::{
  :accepted (i64.const 11)
  :rejected (i64.const 2)
  :ignored (i64.const 3)
  :info (%Bucket::{
    :count (i64.const 10)
    :total (i64.const 100)
    :max (i64.const 17)
  })
  :warn (%Bucket::{
    :count (i64.const 4)
    :total (i64.const 19)
    :max (i64.const 8)
  })
  :error (%Bucket::{
    :count (i64.const 1)
    :total (i64.const 5)
    :max (i64.const 5)
  })
})
```

### T.order

trace（呼出し番号の配列）, left, rightを答える。$markは引数の番号を$traceへ記録し、同じ番号を返す。

```text
(%Pair::{
  :right (call $mark (local.get $trace) (i32.const 2))
  :left (call $mark (local.get $trace) (i32.const 1))
})
```

### T.cell

valueとmutable（構築後にstruct.setできるか、boolean）を答える。

```text
(%Cell::{
  :value (i32.const 40)
})
```

### T.job

index, durationType（i32/i64）, depsNull（boolean）, keyFrom（id/index/duration）を答える。$id:$text="task-7", $index:i32=3, $duration:i64=40という束縛を利用できる。

```text
(%Job::{
  :id (local.get $id)
  :key (%text.bytes (local.get $id))
  :index (local.get $index)
  :duration (local.get $duration)
  :deps (ref.null $Link)
})
```

## 表記U

(%T::{:field expression ...}) は宣言された$Tをフィールド名で初期化する。記述順は任意。値は記述順に各1回評価する。フィールドの期待型が$textなら裸の文字列、i32/i64ならその型の範囲内の整数リテラルを書ける。他の式は通常のWAT/WLT式。

### U.user

name, age, emailの値を答える。

```text
(%User::{
  :name "Alice"
  :age 30
  :email "alice@example.com"
})
```

### U.bucket

count, total, maxの値とtotalType（i32/i64）を答える。

```text
(%Bucket::{
  :count 7
  :total 91
  :max 22
})
```

### U.nested

accepted, warnTotal（warn.total）, errorMax（error.max）を答える。

```text
(%State::{
  :accepted 11
  :rejected 2
  :ignored 3
  :info (%Bucket::{
    :count 10
    :total 100
    :max 17
  })
  :warn (%Bucket::{
    :count 4
    :total 19
    :max 8
  })
  :error (%Bucket::{
    :count 1
    :total 5
    :max 5
  })
})
```

### U.order

trace（呼出し番号の配列）, left, rightを答える。$markは引数の番号を$traceへ記録し、同じ番号を返す。

```text
(%Pair::{
  :right (call $mark (local.get $trace) (i32.const 2))
  :left (call $mark (local.get $trace) (i32.const 1))
})
```

### U.cell

valueとmutable（構築後にstruct.setできるか、boolean）を答える。

```text
(%Cell::{
  :value 40
})
```

### U.job

index, durationType（i32/i64）, depsNull（boolean）, keyFrom（id/index/duration）を答える。$id:$text="task-7", $index:i32=3, $duration:i64=40という束縛を利用できる。

```text
(%Job::{
  :id (local.get $id)
  :key (%text.bytes (local.get $id))
  :index (local.get $index)
  :duration (local.get $duration)
  :deps (ref.null $Link)
})
```

## 回答

各表記について6ケースへ回答し、bucketのtotalだけ100へ変えた構築式全体をeditに書く。他の値や型を変えない。各軸の根拠をevidenceへ記す。未記載の規則や迷いはunclearに列挙する。時間や速度を測ったとは主張しない。コンパイラや他の資料は使わない。

JSON形式:
```json
{"variants":{"R":{"answers":{"user":{"name":"...","age":0,"email":"..."},"bucket":{"count":0,"total":0,"max":0,"totalType":"i32 or i64"},"nested":{"accepted":0,"warnTotal":0,"errorMax":0},"order":{"trace":[],"left":0,"right":0},"cell":{"value":0,"mutable":false},"job":{"index":0,"durationType":"i32 or i64","depsNull":false,"keyFrom":"id or index or duration"}},"edit":"bucketの式全体","scores":{"fields":0,"types":0,"order":0,"layout":0,"changes":0},"evidence":"各軸をcase IDと箇所に対応させた根拠","unclear":[]},"S":{},"T":{},"U":{}},"recommendation":"用途と理由","limitations":["評価の限界"]}
```
S/T/UもRと同じ完全な形式で書く。説明文の長さでは加点しない。
