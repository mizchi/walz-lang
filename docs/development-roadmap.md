# 実用化に向けた実装順

1. 差分・fuzz と CI。seed、最適化有無、マクロ版／明示版、Wasmtime／Node、副作用順、取消しと解放を検査し、失敗の縮小・保存・再実行を用意する。
2. 既存の値型を扱う WIT binding 生成。型は WIT、effect/capability は別の信頼する policy から取得する。
3. 型付きの外部 resource と明示 close。using の範囲、正常・Result.err・例外・取消しの終了経路を検査する。
4. 型付き IR の小さな簡約。直後に match する値の不要な箱を除き、評価順と所有権を保つ。
5. 標準ライブラリの JSON 値と解析を補完し、実例の重複を確認してから単相化ジェネリクスの必要性を判断する。

## 差分検査

`just fuzz SEED CASES` は有限な型付き式を生成する。演算結果と host 呼出し順のハッシュを独立に計算し、マクロ／明示版と最適化有無を Wasmtime と Node で照合する。
不正ソースの変異も検査し、コンパイラの panic を失敗として扱う。失敗は `target/fuzz/failures/` に保存する。
保存された式は `just fuzz-replay FILE` で再実行できる。言語全体や全入力についての健全性の証明ではない。
`runtime/fuzz.test.mjs` は取消しの queued / pending / completion と微小ステップの順序を seed から選び、返却値・取消し理由・二重 close・管理表の残数を検査する。

CI は Rust/Lean/Wasm CLI と JS/Chromium の2ジョブを実行し、失敗入力・ブラウザログを artifact として保存する。ツールの版と配布物のハッシュは `ci/tools.json` に固定する。

## 直後の match の簡約

共有の Result/Pair を構築して直後に match する場合、選択された arm と payload の let 列へ変換する。
使わない payload も左から一度ずつ評価し、trap と副作用を保つ。所有 payload は対象外。
元ソースの型・effect・所有権検査、必要な Lean 検査が先に走る。IR/analysis は元ソースを保持する。
`compile_with_options(..., CompileOptions { optimize: false })` が比較基準となる。
このパスと到達性除去は compiler TCB であり、Lean による意味保存の証明ではない。
