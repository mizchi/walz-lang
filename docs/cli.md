# CLI のインストールと使い方

Rust 1.99以降の Cargo からインストールする。

```sh
cargo install --git https://github.com/mizchi/wlt-lang --locked --bin wlt
wlt --version
```

ローカルのチェックアウトを使う場合は、リポジトリ直下で次を実行する。

```sh
cargo install --path . --locked --bin wlt
```

`wlt` は Cargo のインストール先（通常は `~/.cargo/bin`）に置かれる。このディレクトリを `PATH` に含める。利用時にソースのチェックアウト・Cargo・just・pnpm は必要ない。コアライブラリ、証明カーネルのソース、ベンチ用ドライバはバイナリに埋め込まれる。

| コマンド | 追加で必要なもの |
| --- | --- |
| `wlt build`、`check`、静的解析 | 通常のコンパイルには不要 |
| `wlt lsp` | LSP クライアントを持つエディター。[診断・hover・定義移動](lsp.md) |
| `wlt test`、`run --target core`（既定） | Wasm GC 対応の `wasmtime` が PATH 上に必要 |
| `wlt run --target core-js` | JSPI 対応の Node.js 24以降 |
| `wlt bench` | Node.js 24以降が PATH 上に必要 |
| `verify`、law や `%race-free` を含むビルド | elan 経由の `lean +v4.34.1` が必要 |

`test --list` と `bench --list` は型検査・一覧表示のみで、実行ランタイムは不要。

リポジトリの [最小プロジェクト](../examples/cli/README.md) なら、`cd examples/cli` の後に `wlt build`・`wlt test`・`wlt bench` をそのまま実行できる。

## build

```sh
wlt build
wlt build examples/closure.wlt
wlt build examples/closure.wlt -o dist/closure.wasm
wlt build ./my-project
wlt build main.wlt output.wasm
wlt build main.wlt --target core-js -o dist/app
wlt build main.wlt --target component
wlt build main.wlt --target component --wit api.wit --world guest
```

引数なしではカレントディレクトリの `main.wlt`、次に `src/main.wlt` を探す。設定ファイルや親ディレクトリの探索は行わない。両方ある場合は `main.wlt` を使う。入力を明示すれば通常の WAT も受け付ける。

既定の出力は `target/<入力の拡張子を除いた名前>.wasm`。たとえば `src/main.wlt` は `target/main.wasm`、`foo.test.wlt` は `target/foo.test.wasm` になる。ディレクトリを入力にすると、その中のエントリーと `target/` を使う。

`-o` / `--output` で出力先を指定する。相対出力パスはカレントディレクトリが基準で、親ディレクトリも作成する。従来の `wlt build INPUT OUTPUT` も使用できる。型・effect・所有権・必要な証明と Wasm の検査を通してから出力する。ソースと取り込んだファイルへの上書きは、symlink / hard link の別名も含めて拒否する。

独自のホスト import policy は `--bindings policy.json` で渡す。`--` 以降はファイルパスとして扱う。

既定の出力は Core Wasm。実行する場合は `wlt run main.wlt`、または次のように Wasmtime へ渡す。

```sh
wasmtime run -W gc=y,function-references=y,exceptions=y --invoke main target/main.wasm
```

`--target core-js` は `target/<name>-js/` に `index.js`・Wasm・codec・ランタイムを同梱する。ビルド後の実行にチェックアウトや Cargo は不要。`const {instantiate}=await import('./dist/app/index.js')` から呼べる。Node とブラウザの JSPI に対応する。
`--target component` は `target/<name>.component.wasm` と manifest を生成する。JS への変換は固定版 jco を使う開発タスク `just build-js` を参照。旧 `component` / `core-js` サブコマンドも低水準の生成用に残る。[ランタイム](runtime.md)。

## run

```sh
wlt run
wlt run main.wlt --invoke add -- 40 2
wlt run main.wlt --target core-js --invoke main -- 40 '"2n"'
wlt run main.wlt --target core-js --stdio --input input.txt --output output.txt
wlt run main.wlt --target core-js --bindings generated/bindings.json --host host.mjs
wlt run main.wlt --exit-code
```

エントリーの探索は build と同じ。既定 export は `main`、`--invoke` で選択する。`--` 以降はプログラム引数。core は Wasmtime の数値引数、core-js は export が引数を取る場合に JSON 値として解釈する。BigInt は `"42n"`。引数を取らない export はコマンドライン引数を関数引数として受け取らない。

core-js では常に `wlt:host/cli@0.1.0.arguments` の `list<string>` として元の文字列引数も参照できる。標準入出力は `--stdio` で明示的に許可する。`--input` / `--output` はそれぞれ標準入力・出力の代わりのファイル。stdio モードは戻り値を stdout に追加しない。

`--host` のモジュールは options オブジェクト、または `({args,signal}) => options` を default export する。`imports` と `eventLoop` に必要なホスト実装を供給する。資源は host の `Symbol.asyncDispose` でも解放できる。通常は値を stdout へ表示し、`result.err` は stderr と終了コード1。`--exit-code` は 0–255 の整数結果を終了コードにする。

ファイルは `--read-file 名前=ホストパス` / `--write-file 名前=ホストパス` を必要な数だけ指定する。guest が使えるのは指定した名前だけ。ソース・入力への出力上書きはファイルの別名も含めて拒否する。[WASI ファイル adapter](wasi-files.md)。

割り込みでは Node の AbortSignal を介して guest を取り消し、ホストと I/O の解放を待つ。Unix の親プロセスだけに SIGINT / SIGTERM を送った場合も子へ転送し、終了を待って一時成果物を削除する。取消しの終了コードは130。

### 戻り値と実行先の選択

| 用途 | コマンド | stdout |
| --- | --- | --- |
| 数値やunitを返すCore Wasm | `wlt run main.wlt` | Wasmtimeの値表示 |
| string・list・Resultなどの共有値をJSで受け取る | `wlt run main.wlt --target core-js` | JSONでエンコードした値。stringなら引用符も出力 |
| パイプやファイルへraw textを出す | `wlt run main.wlt --target core-js --stdio` | 明示的なI/O importが書いた内容。戻り値を追加しない |
| WIT interfaceとしてホストへ組み込む | `wlt build main.wlt --target component --wit api.wit` | Componentを生成。ホストで実行する |

CoreのGC参照の表示は `<anyref>` 等になり、stringの本文へ自動変換しない。`--stdio` は戻り値をraw textへ変えるオプションではなく、プログラムが要求する標準入出力capabilityを許可する。
同じファイルの `%test` / `%bench` は通常のJS/Componentビルドの公開ABIから外す。宣言の型・effect・所有権はその場合も検査し、実行と計測は `wlt test` / `wlt bench` で選択する。

## test と bench

```sh
wlt test
wlt test tests/programs --filter closure
wlt test --list
wlt test tests/example.test.wlt --timeout-ms 1000

wlt bench
wlt bench examples --filter JSON --samples 20
wlt bench --list
wlt bench --json --samples 3 --iterations 100 --warmup-ms 0
```

`test` は既定で `tests/` の `*.test.wlt`、`bench` はカレントディレクトリ以下の `*_bench.wlt` を再帰探索する。ファイルを直接指定した場合は名前を問わない。複数のパスを渡せ、同じファイルは1回だけ対象になる。

再帰探索では `target`・`node_modules`・`.git` と symlink をたどらない。これらのディレクトリやファイルを入力として明示すれば対象にできる。`--` 以降のパスはオプションとして解釈しない。

各テスト・ベンチは独立したプロセスで実行し、失敗・trap・タイムアウト後も他のケースを続行する。0件の場合は失敗する。宣言方法は [テスト](testing.md) と [ベンチマーク](benchmarking.md) を参照。

## help と終了コード

```sh
wlt --help
wlt build --help
wlt test --help
wlt bench --help
wlt help build
wlt --version
```

終了コードは成功が0、ソース・実行環境のエラーやテスト／ベンチの失敗が1、引数の誤りが2。

CLI 開発時には `just test-cli`、パッケージからのインストール検査には `just test-install` を使う。後者は一時領域へインストールし、ビルド元を削除して別ディレクトリで3コマンドと失敗時の終了コードを検査する。通常のインストールは `just install` でも実行できる。

## マクロ展開と型の確認

```sh
wlt check --json examples/user-macros/main.wlt
wlt macro-expand examples/user-macros/main.wlt
wlt macro-expand --text examples/user-macros/main.wlt
wlt expand examples/user-macros/main.wlt
```

`macro-expand` は型検査を通した展開履歴を出し、ゲストを実行しない。既定は `wlt-macro-expansion-v1` のJSONで、宣言・展開元・推論型・入力の利用状況・`checking` の仕事量を含む。`--text` は展開元・推論型・入力の構文出現数を人が読む形式で表示する。`expand` は全拡張を下げた最終WATを出す。

`single` / `duplicated` / `discarded` は入力構文の出現数の分類で、ループや分岐を含む実行回数ではない。`checking` のカウンタも検査予算の指標であり、CPU時間ではない。構文の使い分けと上限は [ユーザー定義マクロ](user-macros.md)、編集時の型・定義・展開照会は [LSP](lsp.md) を参照。

## Core＋JSPI のテスト・計測

`wlt test --target core-js --bindings policy.json --host host.mjs` と
`wlt bench --target core-js --bindings policy.json --host host.mjs` で、非同期 import と子タスクを実行できる。
ケースごとにプロセス・Wasm instance・host factory を作り、正常・失敗・取消し後にホストの解放を待つ。
`--list` は型チェックだけを行い、host module と Node を起動しない。
タイムアウト時は協調的な取消しを要求し、最大5秒の解放猶予後も終了しないホストは強制終了して失敗を報告する。

Core＋JSPI のベンチは、各呼出しを順番に await する end-to-end 計測。codec・JSPI・待機時間を含み、結果は ABI 対応の共有値または unit に限定する。
既定の `core` の Wasm 内バッチ計測とは別の測定条件で、JSON の `options.target` と `summary.runtime` に区別を残す。

## 実行期限

`wlt run --target core-js --timeout-ms 1000` は guest の start からの期限を設定する。
コンパイル・host factory の初期化はこの期限に含めない。時間切れは終了コード124、
外部からの取消しは130で、どちらも guest と host の解放を待って終了する。
同じ instance の別の呼出しや、WLT の子タスクの扱いは [スコープ](scoped-tasks.md) を参照。
`test` / `bench` の期限はケースの Node 初期化も含み、応答しない provider には5秒の解放猶予後にプロセスを終了する。
