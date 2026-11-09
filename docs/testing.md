# 言語内のテスト

性能計測は `*_bench.wlt` の `%bench` と `wlt bench` を使う。[ベンチマーク](benchmarking.md) を参照。

標準 API 資料は `just stdlib-docs` で生成する。`just test-stdlib-docs` は公開シグネチャ・制約・effect と資料の一致、および非公開な型表現を含まないことを検査する。

モジュール直下に名前と `i32` の条件式を置きます。0 が失敗、それ以外が成功です。

```wat
(module
  (func $double (param $n i64) (result i64)
    (i64.mul (local.get $n) (i64.const 2)))

  (%test "double returns 42"
    (i64.eq (call $double (i64.const 21)) (i64.const 42))))
```

```sh
just test-language
just test-language tests/programs GC
just list-tests tests/programs
wlt test tests/programs --filter closure
wlt test tests/programs/core.test.wlt --timeout-ms 1000
```

`wlt test` は既定で `tests/` 配下の `*.test.wlt` を再帰探索します。ファイルを明示した場合は名前を問いません。
複数のパスを渡せ、同じファイルは1回だけ実行します。ディレクトリ探索ではシンボリックリンクをたどりません。
`target`・`node_modules`・`.git` は再帰探索から除外し、入力パスとして明示した場合だけ探索します。[インストールとCLI](cli.md) も参照してください。
`--filter` はテスト名の部分一致、`--list` は型検査と一覧表示のみです。絞り込み前にモジュール全体を型検査します。
失敗、trap、コンパイルエラーがあっても他のテスト・ファイルを続けます。0件の場合も成功扱いにしません。
終了コードは成功が0、テスト失敗・実行環境やソースのエラーが1、引数の誤りが2です。

各テストは新しい Wasmtime プロセスとインスタンスで実行します。既定の制限時間は起動・コンパイル込みで1本5秒です。
`--timeout-ms` で変更できます。タイムアウト時はプロセスを終了し、次のテストを実行します。実行には PATH 上の Wasmtime が必要です。
テスト失敗時の実行時診断は最大64 KiBを表示します。import の実装は通常の run と同様にランナーでは提供しません。

テスト本体も通常の型付き IR へ変換し、Wasm GC 上で実行します。暗黙の capture・共有可変状態・動的な関数レジストリは追加しません。
GC の割当て、参照同一性、明示した変更、trap、評価順序は通常のコードと同じです。型・副作用の検査と定義域は [SPEC.md](../SPEC.md) を参照してください。
[core.test.wlt](../tests/programs/core.test.wlt)・[gc.test.wlt](../tests/programs/gc.test.wlt)・[macros.test.wlt](../tests/programs/macros.test.wlt)・[values.test.wlt](../tests/programs/values.test.wlt)・[ownership.test.wlt](../tests/programs/ownership.test.wlt)・[delivery.test.wlt](../tests/programs/delivery.test.wlt) の計21本は、`just test` からも検証します。
Rust API は `wlt::runner::run(paths, options)` で、各テストの結果・ファイル単位のエラーを構造化して取得できます。

## Core＋JSPI のテスト・計測

`wlt test --target core-js --bindings policy.json --host host.mjs` と
`wlt bench --target core-js --bindings policy.json --host host.mjs` で、非同期 import と子タスクを実行できる。
ケースごとにプロセス・Wasm instance・host factory を作り、正常・失敗・取消し後にホストの解放を待つ。
`--list` は型チェックだけを行い、host module と Node を起動しない。
タイムアウト時は協調的な取消しを要求し、最大5秒の解放猶予後も終了しないホストは強制終了して失敗を報告する。

Core＋JSPI のベンチは、各呼出しを順番に await する end-to-end 計測。codec・JSPI・待機時間を含み、結果は ABI 対応の共有値または unit に限定する。
既定の `core` の Wasm 内バッチ計測とは別の測定条件で、JSON の `options.target` と `summary.runtime` に区別を残す。

## ユーザー定義マクロの回帰と評価

```sh
just test-user-macros
just test-macro-tools
just test-macro-composition
just fuzz-user-macros 20261109 64
```

構文・衛生性・推論・LSPのほか、反復束縛・委譲、式からの検査再開、推論の仮ローカルとラベル、深い展開の拒否を検査する。差分fuzzはマクロ／独立した手書き展開／値と作用のモデルを、最適化あり・なし、Wasmtime／V8で照合する。最初のタスクにはCore／ComponentとChromiumの検査も含み、Node.js 24以降とインストール済みの開発依存が必要。

ユーザーが書きたい処理を実装できるか、診断が次の修正につながるか、規模を増やすとどうなるかは [マクロの実用性評価](../evals/README.md#マクロの実用性と規模の評価)で別に確認する。必須の正しさと未達の探索課題を分け、固定した入力・期待値・ルーブリックは上書きしない。
