# JSON parser tuning: 2026-11-06

最終結果は`report.json` / `report.md`。初回の`20261106-r2`と採用した`20261106-r7-unroll`を同じNodeプロセスで交互に9回測定した。
手順と判断は[チューニングの説明](../../../docs/json-parser-tuning.md)に記録した。

`capture.json`は採用版の全生成物・入力・ソースのSHA-256を記録する。`corpus.json`は固定した9入力。
`buffers.rs`は採用したコンパイラのbytes helperで、各試行の`json-value.wlt`はその時点のライブラリの写し。
ここにあるのは結果・入力・計測コード・関連ソースであり、全バイナリを含むcaptureではない。
実行可能な全captureは`target/json-parser-bench/20261106-r2`と各試行のディレクトリに保存している。

| ディレクトリ | 変更前 → 変更後 | 回数 | 判断 |
|---|---|---:|---|
| scalar | r2 → r3-scalar | 5 | 重複load削減、通常文字列のbuilder省略、GC一括copyを採用 |
| chunks | r3-scalar → r4-chunks | 5 | 32要素chunkによる構築。一部の配列は改善、レコードは悪化し不採用 |
| simd | r3-scalar → r5-simd | 5 | GCから16byteをregisterへ詰めて分類。短い文字列が悪化し不採用 |
| simd-prefix | r3-scalar → r6-simd-prefix | 5 | スカラーprefixの後にSIMDを使う版。レコードが悪化し不採用 |
| unroll | r3-scalar → r7-unroll | 5 | スカラーの8byte展開を採用 |

各試行の全パスは`target/json-parser-bench/20261106-<試行名>`。
値の照合・ASTの再シリアライズhash・ABI未解放数0を全測定で検査した。
最終版は`just check`、Rust 327、GC有効のNode 83、Chromium 48テストを通過した。

保存した計測コードによる再実行例:

```sh
node --expose-gc benchmarks/json-parser/20261106-r7-tuning/compare.mjs \
  target/json-parser-bench/20261106-r2 \
  target/json-parser-bench/20261106-r7-unroll \
  target/json-parser-bench/new-comparison 9
```

出力先は未使用のディレクトリを指定する。元のcaptureがない環境では、バイナリを含むcaptureを用意する必要がある。
