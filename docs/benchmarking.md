# ベンチマーク

`*_bench.wlt` に `%bench` を書き、`wlt bench` で実行する。Node.js 24 以降が必要。
Rust側の型検査・コンパイル時間は [Criterionによるフロントエンドの計測](frontend-performance.md) を参照。

```wat
(module
  (%bench "integer mix"
    (param $i i32)
    (result i32)
    (^ (* $i 16777619) (>>> $i 7))))
```

```sh
just bench                          # カレント以下の *_bench.wlt
just bench examples 'JSON'          # 名前の部分一致
just list-benches examples
cargo run --release -- bench examples/benchmarks/stdlib_v2_bench.wlt --samples 20
cargo run --release -- bench examples --json > target/bench.json
cargo run --release -- bench examples --iterations 1000 --warmup-ms 100
just test-bench
```

名前と本体は必須。名前はモジュール内で重複できず、空白だけの名前も拒否する。
`(param $i i32)` と `(result 型)` は、この順でそれぞれ1個まで省略可能。
結果型を省略すると unit になり、`(%bench "unit" (nop))` のように書ける。
本体は通常の型・effect・所有権検査を受ける式1個で、ヘルパーは通常の `func`、
ライブラリは明示した `%use` で利用する。bench 宣言は実行用のエントリーファイルに置く。
ライブラリとして import するファイルの中には置けない。

`i32` の反復番号は各バッチで 0 から始まる。バッチ内の全反復で本体を1回ずつ呼ぶ。
省略すれば引数なしの本体になる。条件を変えずに比較する場合は `--iterations` を固定する。

返した値は Wasm の公開 global に保存し、計算結果が観測可能なままにする。
GC参照や `v128` も返せる。保持するのは最後の1個で、過去の結果は到達不能になれば回収できる。
所有値や borrow は返せない。資源は本体の中で消費・close し、共有値または unit を返す。
unit の本体で計算結果を `drop` すると、その計算は通常どおり最適化で消えることがある。
返り値の保持は定数畳み込みやループ不変式の移動を禁止しないため、測りたい入力は反復番号などで変化させる。

## 計測と表示

ケースごとに別の Node プロセスと Wasm インスタンスを作る。その中では校正、ウォームアップ、
各サンプルを同じインスタンスで実行する。WLT/Wasmのコンパイル、プロセス起動、
インスタンス生成、校正、ウォームアップはサンプルの時間に含まない。

反復は Wasm 内のループで実行し、JS はバッチの前後だけで単調時計を読む。
結果はバッチ時間を反復数で割った ns/op。反復ループ・結果保存・バッチ単位の JS/Wasm 呼び出しの負担、
GCとJITの影響は含む。極小の式ではこの負担が支配的になるため、実用的な処理単位で測る。
固定したリテラルの構築も本体にあれば計測対象になる。

| オプション | 既定値 | 意味 |
| --- | --- | --- |
| `--samples N` | 10 | 測定バッチ数。1〜10000 |
| `--sample-ms N` | 10 | 自動校正する1バッチの目標ミリ秒。正の整数 |
| `--iterations N` | 自動 | 1バッチの反復数。1〜2147483647。指定時は校正を省略 |
| `--warmup-ms N` | 100 | 計測前のウォームアップ時間。0で省略 |
| `--timeout-ms N` | 30000 | ケース単位の制限時間。Node起動・Wasmコンパイル・校正も含む |
| `--filter TEXT` | なし | bench名の部分一致 |
| `--list` | なし | 全体を型検査し、対象だけ一覧表示。Nodeは不要 |
| `--json` | なし | `wlt-bench-v1` の構造化結果 |

自動校正では反復数を倍増し、目標時間に達するか上限に達したところで固定する。
表示は中央値、平均、最小、最大、標本標準偏差と、サンプル数・反復数。
JSONには測定順の生サンプルとNode/V8の版、OS・アーキテクチャ、オプション、失敗理由も残す。
外れ値は削除・加工しない。比較する環境、入力、オプションと結果は一緒に保存する。

実行対象は閉じた同期的なguestコード。ホストimportの実装、JSPI/Componentの非同期処理はランナーでは提供しない。
選んだbench以外のexportはリンクの根から外すため、無関係なimportは持ち込まない。
通常の `%test` と探索・実行を分け、`wlt test` はbenchを実行しない。

ディレクトリは再帰探索し、シンボリックリンクをたどらない。明示したファイルは名前を問わず、
重複は正規化したパスでまとめる。`--` の後はすべてファイルパスとして扱う。
trapやタイムアウト、ファイルの型エラーがあっても他のケースは続行する。
0件はエラー。終了コードは成功0、計測・ソース・環境のエラー1、引数の誤り2。

Rust APIは `wlt::benchmark::run(paths, options)`。検査済みIRからハーネスを作る場合は
`wlt::benchmark::compile(source, &module, index)` を使う。
[標準ライブラリの計測例](../examples/benchmarks/stdlib_v2_bench.wlt) に整数、ビュー、JSON解析をまとめている。

[MoonBitのbench実装](https://github.com/moonbitlang/core/blob/main/bench/bench.mbt) の
バッチ反復・単調時計・結果保持を参考にした。WLTでは専用のbenchオブジェクトを導入せず、
宣言とCLI、観測可能な返り値で表す。

## プロファイルを使ったstdの改善

[最初の改善結果と固定した計測データ](../benchmarks/stdlib-json/20261106-v1/report.md) は、JSONエスケープの変更前後を記録している。

変更前の `%bench` をWasmとして固定し、プロファイルと速度比較を別の実行で取る。
例は [std_tune_v2_bench.wlt](../examples/benchmarks/std_tune_v2_bench.wlt)。64／1024バイトの連結、
ASCII・Unicode・エスケープを含むJSON化、builder、JSON解析と、入力構築だけの対照を含む。
リテラルの構築も本体に含むため、処理だけの時間とみなさない。
v2は `$json.stringify` と `$json.from-string` を使う。旧APIの入力は `stdlib-v1.wlt`／`std-tune-v1.wlt` に元の内容を保持し、自動探索から外す。
v1の測定は保存済みWasmで再実行する。APIと入力の変わったv2をv1と同じ比較組として扱わない。

```sh
just bench-capture examples/benchmarks/std_tune_v2_bench.wlt target/std-tune/before
just bench-profile target/std-tune/before target/std-tune/profile-before 'json.stringify string ASCII 1024' 200000

# stdを修正し、機能検査を通した後
just test-std-tune
just bench-capture examples/benchmarks/std_tune_v2_bench.wlt target/std-tune/after
just bench-compare target/std-tune/before target/std-tune/after target/std-tune/comparison.json
just bench-profile target/std-tune/after target/std-tune/profile-after 'json.stringify string ASCII 1024' 200000
```

captureはWasm、入力、展開済みソース、計測driver、stdlibソースを保存し、SHA-256を記録する。
ディレクトリや結果ファイルは上書きしない。profile／compareの前に記録と実ファイルを照合する。
比較する入力やdriverが異なれば拒否する。計測には固定したdriverとWasmを使う。
測定後の入力を変える場合は別のsuite・snapshotとして保存し、基準値から取り直す。

compareは基準側で校正した同じ反復数を両方へ渡し、ウォームアップ後にbefore→after／after→beforeを
交互に実行する。生サンプル、対応するサンプル比の中央値、Wasmサイズ、バイナリが同一かどうかを保存する。
スカラー結果は一致を検査する。GC参照の内容はこの比較では検査できないため、別途oracleによる機能検査を行う。
`just test-std-tune` は正確なJSONエスケープ、Unicode、連結・builder、ABI解放をCoreとComponentの両方で検査する。

profileは同じWasmをV8とWasmtimeで別々に実行し、Chrome DevTools等で読める `.cpuprofile`、
Firefox Profilerで読める `.guest.json` と、関数ごとのサンプル集計を出す。
V8はウォームアップ後に約1秒、Wasmtimeは指定した反復数を1ms間隔でサンプリングする。
プロファイルの起動・Wasmコンパイルは採取区間から外し、プロファイル下の時間を速度比較には使わない。
Wasmtime側のサンプルが少なければ、新しい出力先で反復数を増やす。
両エンジンでインライン化やGCが異なるため、サンプル比をエンジン間で性能比として扱わない。
