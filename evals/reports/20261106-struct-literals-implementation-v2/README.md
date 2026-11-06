# `$`を保持する短縮形の実行確認

短縮形を`%$Type::`へ揃えた実装で、同じ6パターンの4表記を再コンパイル・実行した。
すべて42を返し、生成されたWasmは[前回の実装確認](../20261106-struct-literals-implementation-v1/README.md)と表記ごとに完全一致した。

`support/cases.mjs`に固定した現在のケース生成器を保存した。`baseline()`内の各位置引数式を、対応する`render(expression, variant)`で置き換えて各プログラムを生成した。
コンパイラ・ケース・ソース・Wasmのhashと結果は[verification.json](verification.json)に記録した。
バイナリは`target/struct-dollar-execution`に保存している。過去の評価入力・回答・結果は書き換えていない。

```sh
just run evals/reports/20261106-struct-literals-implementation-v2/objectShort.wlt
```

値の型付けは変えていない。裸リテラルはフィールド宣言を期待型としたWasm定数命令へ展開する。
型の異なる2つのフィールドへ同じ`30`を置いた場合に、明示的な`i32.const` / `i64.const`と同じWasmになることもRustの回帰テストで検査した。
これは実装・命令の一致の確認であり、新たな読解役による可読性評価ではない。
