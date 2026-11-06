# 所有権と証明を通して実行する

```wat
(%let $ticket (%own.new i64 (i64.const 42))
  (%match (%inst $own i64) (result i64) (%move $ticket)
    (case own ($value) (local.get $value))))
```

`(%inst $own T)` は複製できない包みです。move 後の再利用、暗黙の放棄、共有 GC フィールドへの格納を拒否します。
Option/Result/pair と捕捉するクロージャにも所有性が伝播します。使わない包みは `(%own.close (%move $ticket))` で明示的に消費します。
内容を match で取り出すことも消費です。own は payload の GC 参照を排他的にする型ではなく、own.close は外部リソースの終了処理をまだ実装していません。
borrow/lifetime は設けず、操作に渡した所有値は戻り値の一部として受け直す方針です。[ownership.wlt](../examples/ownership.wlt) に組と once-call クロージャの例があります。

```wat
(%law "adding zero preserves the value" (param $x i32)
  (i32.eq (call $add (local.get $x) (i32.const 0)) (local.get $x)))
```

law は i32/i64 の全入力で条件が非0になることを要求し、実際の関数本体から生成した BitVec の証明を Lean で検査します。
整数の折り返し・符号・Wasm のシフト量のマスクを含めます。初版の law は純粋な整数式と非再帰の直接呼び出しに限定します。
関数本体を変更して law が壊れると、通常の run/build も拒否します。検証器の不在や、未対応の意味論もエラーになります。
[verified-laws.wlt](../examples/verified-laws.wlt) は加算の law と所有値を組み合わせた実行例です。

`run-verified` は所有権の契約も独立した Lean の検査器へ渡し、通った同じ IR から Wasm を生成します。
このモードでは未解析の WAT、import、所有値を渡す raw export、effect 上限のない公開関数を拒否します。通常の WAT の受付は維持します。
Rust API `check` と調査用の types/ir/analyze は law を証明しません。CLI check と実行形式の生成は、law があれば証明を検査します。
形式化した範囲と信頼する実装は [SPEC.md](../SPEC.md#law-と契約付き実行)・[proof/README.md](../proof/README.md) に記載しています。コンパイラ全体の意味保存や、停止性の証明は含みません。
