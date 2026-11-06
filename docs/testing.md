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
cargo run --locked -- test tests/programs --filter closure
cargo run --locked -- test tests/programs/core.test.wlt --timeout-ms 1000
```

`wlt test` は既定で `tests/` 配下の `*.test.wlt` を再帰探索します。ファイルを明示した場合は名前を問いません。
複数のパスを渡せ、同じファイルは1回だけ実行します。ディレクトリ探索ではシンボリックリンクをたどりません。
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
