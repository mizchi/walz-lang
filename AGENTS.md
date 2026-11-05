# WLT の開発

ユーザーには日本語で答える。

- 探索 → Red → Green → Refactoring の順で開発する。変更する振る舞いを先に検査する。
- タスクは `justfile` に置く。JavaScript は Node.js 24 以降と pnpm、ブラウザ検査は Playwright を使う。
- 状態とロジック、公開 API・型の契約と実装を分離する。小さい仕様と静的解析のしやすさを優先する。
- 標準 WAT を受け付ける性質を維持する。拡張を Wasm の型・例外・GC の意味に沿って展開する。
- 型・effect・所有権の検査を省略して最適化しない。Lean の契約を変更した場合は生成側・モデル・検査をそろえる。
- GC オブジェクト、ABI 変換用の線形メモリ、ホストの外部資源を区別する。取消し・例外・正常終了の解放経路を検査する。
- ランタイムの変更は Core＋JSPI と Component の両方で検査し、ブラウザに関わる変更は E2E でも確かめる。
- 評価の入力・ルーブリック・結果・計測スナップショットを後から書き換えない。課題を変更したら suite の版を上げ、新しい run を作る。

主な検査は `just check`、`just test`、`just test-language`、`just test-js`、`just test-e2e`、`just test-evals`、`just proof-check`、`just wit-check`。
`just test-workflow` は GC 到達性も検査する。改名前の記録と環境の詳細は [MIGRATION.md](MIGRATION.md) を参照する。
