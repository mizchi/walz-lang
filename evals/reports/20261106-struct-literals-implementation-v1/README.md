# struct構築表記の実行確認

名前付き構築を実装した後、読解評価v2の固定した`support/cases.mjs`から、4表記の確認用プログラムを生成した。
各プログラムに6ケースを入れ、コンパイル・Wasm検証・Wasmtime実行を行い、いずれも42を返した。
生成には、固定した`baseline()`中の各位置引数式を、同じケースの`render(expression, variant)`へ置換した。
これは候補構文の実行確認であり、新しい読解役による可読性の再評価ではない。

| プログラム | 表記 | 結果 |
|---|---|---:|
| [positional.wlt](positional.wlt) | 標準WATの位置引数 | 42 |
| [named.wlt](named.wlt) | 名前付きS式 | 42 |
| [objectExplicit.wlt](objectExplicit.wlt) | 波括弧＋明示リテラル | 42 |
| [objectShort.wlt](objectShort.wlt) | 波括弧＋期待型リテラル | 42 |

```sh
just run evals/reports/20261106-struct-literals-implementation-v1/named.wlt
```

[verification.json](verification.json)に固定ケース・ソース・Wasm・使用したコンパイラのhashを保存した。
baseCommitは実装前のHEADで、実装の状態はsourceHashesが示す。実行バイナリはtarget以下にある。
既存のチューニング済みJSONパーサーも再コンパイルし、r7のWasmバイナリと完全一致することを確認した。

ABI・例外・早期脱出・Future待機・所有権・型エラーなどの回帰検査は、別途`just test-structs`と全体テストで実行する。
導入前の入力・回答・点数は[v1](../20261106-struct-literals-v1/REPORT.md)と[v2](../20261106-struct-literals-v2/REPORT.md)から変更していない。
