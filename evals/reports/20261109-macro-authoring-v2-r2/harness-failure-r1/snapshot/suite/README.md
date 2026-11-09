# マクロ評価 v2

```sh
just eval-macro-authoring trial-01
```

現行の型引数推論・型情報API・束縛の合成を使う追加評価。課題と期待値を実行前に固定し、受理されたコードは Wasmtime と V8 の両方で実行する。既存の `just eval-user-macros ID` は別の新しいIDで併走させ、ABI・取消し・stdio LSP の回帰を調べる。異なる課題の点差は比較しない。

`required` は現行の契約、探索課題は次の設計候補。未対応の拒否と、受理したコードの誤動作を区別する。処理系はこの評価中に修正しない。出力は `evals/runs/ID`。保存済みの入力・結果は上書きしない。
