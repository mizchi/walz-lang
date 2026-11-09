# ブラウザーのプレイグラウンド

RustのWLTコンパイラをwasm-packでWasmにビルドし、ブラウザー上で編集・コンパイル・実行する。
コンパイルのためにソースをサーバーへ送信しない。JavaScriptのバンドラーやサーバー側のRust実行は不要。

[公開版を開く](https://mizchi.github.io/wlt-lang/)。画面のDocumentationからREADMEと各言語機能のガイドへ進める。

## 起動とビルド

Rust 1.99以降、Node.js 24以降、pnpm、just、wasm-packと、Wasm GC対応のブラウザーを使用する。

```sh
cargo install wasm-pack --locked
rustup target add wasm32-unknown-unknown
pnpm install --frozen-lockfile
just playground
# http://127.0.0.1:41740
```

ビルドだけを行う場合は`just playground-build`（または`pnpm playground:build`）を使う。
`target/playground/`にHTML・CSS・ES modulesと`pkg/wlt_web_bg.wasm`を生成する。
このディレクトリ全体を任意の静的HTTPサーバーへ配置でき、サブディレクトリ配信にも相対URLで対応する。
`.wasm`を`application/wasm`、`.mjs`と`.js`をJavaScriptのMIME typeで配信する。`file://`ではなくHTTPで開く。
手元のビルド済み成果物は`pnpm playground`で配信できる。

wasm-packは`--target web --release`を使用する。初回は対応するwasm-bindgenのツールも取得するため時間がかかる場合がある。
追加のBinaryen依存を避けるため、このクレートではwasm-optを無効にしている。Rustのrelease最適化は有効。
生成JSとWasmの組み合わせは同じビルドのものを配信する。
オプションの詳細は[wasm-packのbuild資料](https://rustwasm.github.io/docs/wasm-pack/commands/build.html)を参照。

## GitHub Pagesへの公開

[Pagesワークフロー](../.github/workflows/pages.yml)は、`main`へのpushで同じコミットのコンパイラをWasmにビルドし、サンプル・Rustアダプター・Chromiumの検査後に`target/playground/`を公開する。pull requestではビルドと検査だけを行い、公開はしない。Actionsから手動実行もできる。

公開元はリポジトリのSettings → PagesでGitHub Actionsに設定する。生成物をソースブランチへコミットする必要はない。ページ内のサンプルや説明は英語、詳細な言語資料はリポジトリ内のMarkdownを参照する。

## 対応範囲

- 単一ファイルのWLT・標準WAT・同梱された`wlt:core`のコンパイル。
- 通常の汎用呼び出しの型引数推論、衛生的なユーザーマクロ、公開型情報の問い合わせと明示的な束縛スコープ。
- CLIと共通の名前・型・effect・所有権検査、最適化・DCE、Wasm validator。
- 生成WAT、解析JSON、エラーコードと位置・関連位置・note・helpを含む診断の表示。
- 最適化の切り替えとCore Wasmのダウンロード。
- Core Wasmの、引数なしの関数exportの実行。`i64`は`42n`、複数戻り値は配列、戻り値なしは`unit`と表示する。
- `wlt:host/event-loop@0.1.0`の`sleep`・`yield`・`defer-i64`・`ticks`・`fallible-i64`。Wasm GCとWebAssembly JSPIに対応したブラウザーで実行する。

GC参照はWasmの参照として扱い、その内容をJavaScriptから展開しない。
ファイルimport/include、ファイル・ネットワーク等のホストI/O、Component自体の実行はこの画面からは提供しない。
イベントループ以外のホストimportがあるモジュールもコンパイルと保存はできるが、実行時に不足するimport名を表示する。
引数の必要なexportは実行時に診断する。Core＋JSPI／Componentの追加のホスト連携はCLI側から利用できる。
コンパイラが受理したWasmの機能にブラウザーが未対応の場合も、WAT表示とダウンロードは利用でき、実行できない理由を表示する。

Leanの外部カーネルが必要な`%law`や検証付きコンパイルはブラウザーでは利用できない。
検証を省略して生成することはせず、`E_HOST_UNAVAILABLE`を返す。

## 言語機能のサンプル

Exampleメニューから44個のサンプルを選べる。説明と期待結果は画面と同じく英語で表示する。

| グループ | 数 | 紹介する機能 |
| --- | ---: | --- |
| Basics | 3 | 複数束縛、型付きリテラル、演算子、短絡論理、ビット演算、浮動小数点と明示変換 |
| Control flow | 5 | if、block、return、br、状態を渡すloop、for、相互再帰、Wasm例外 |
| Data and types | 8 | 名前付きstruct、GC配列、enumと網羅的match、Resultとdo記法、Option、tuple・フィールドの分解、for-each、フィールド更新、aliasとnewtype |
| Functions and modules | 5 | 明示capture、型引数推論と単相化、型クラス制約、ファイルモジュール、WIT付きComponentの%use、opaque、所有値と一度きりのclosure |
| Syntax extensions | 6 | 衛生的マクロ、推論した型の利用、反復束縛と委譲、公開selector、初期化式・反復位置・body転送の診断 |
| Core library | 7 | 不変listのmap/fold、callable制約、string/bytes、ゼロコピーのstring/bytes/array-view、dict、JSON、v128/SIMD |
| Ownership and async | 4 | 配送済みFutureと有限Stream、fold-result、端点のclose、ホストの中断境界、resourceとborrow |
| Checks and interop | 6 | test、bench、型・move・effectの診断、lawの検証境界、標準WATと複数戻り値 |

34個はそのままRunで実行でき、`Expected output`と実際の結果を比較できる。
ホストFutureの例も、JSPIによるPromiseの待機をブラウザー内で実行する。
残る10個は`Expected diagnostic`を示す例。型・所有権・effectの誤り、マクロのスコープと反復の制約に加え、ファイル解決が必要なモジュール、
resourceに必要な信頼済み`--bindings`設定、lawに必要なLeanカーネルというブラウザー側の制約を紹介する。
resourceの例はホストimportの宣言だけでは権限が成立しないことを示しており、検査を通すための仮の権限は与えない。

配送済みFuture／Streamの例はローカルに構築した値を読み、言語内のproducerやイベントループは開始しない。
閉じる例も既に配送済みの端点を放棄するもので、pendingなホストtaskの取消しとは区別する。
ファイル%use/includeの解決は単一ファイルの画面では扱わない。モジュールの例は2ファイル分の書き方とCLIの利用方法を示し、画面では`E_SOURCE_REQUIRED`を期待する。
WIT付き`.wasm`の例もCLI向けで、画面からのアップロード・Component実行には対応しない。[Wasmの%use](wasm-use.md)に出力先・同期値・取消しの制限をまとめている。

入力を編集するとサンプルの期待結果を消し、`Custom source`に切り替える。
別のサンプルを選べば説明・期待結果・ソースが一緒に戻る。

## 処理の分離

`crates/wlt-web`は既存の`wlt::compile_with_options`を呼ぶ薄いwasm-bindgenアダプター。
`compile_source(source, optimize)`は`wlt-playground-v1`形式のJSON文字列を返す。
成功時は`status: "success"`と`wat`・`wasm`（バイト配列）・`analysis`、失敗時は`status: "error"`と既存の`diagnostic`を返す。

イベントループのimportがある場合は`prepare_execution(source, optimize)`で既存のCore＋JSPIアダプターをメモリ上に生成する。
応答は`wlt-playground-execution-v1`で、成功時の`program`にCore Wasm、ABI用メモリのWasm、生成bindings、manifest、exportの名前と引数個数を含む。
実行Workerが既存の`runtime/core.mjs`・`host.mjs`・`tasks.mjs`・`resources.mjs`を使い、PromiseやAsyncIterableと接続する。
生成bindingsはBlobのES moduleとして読み込んでURLを解放するため、配信時にCSPを設定する場合はWorker内の`blob:` module importを許可する必要がある。
保存するCore Wasmは従来どおりで、ダウンロードしたファイル単体にJavaScriptホストやアダプターは同梱しない。

ブラウザー向けアダプターはRustの型・effect・所有権検査、信頼するimport、WIT境界とexportの契約を確認する。
外部Leanカーネルによる独立検証を行ったとは扱わず、lawを含む入力は拒否する。CLIの検証付きコンパイルは引き続きLeanを呼び出す。

画面と実行時のメッセージは英語で提供する。
コンパイラは再利用するWeb Workerの中に置き、標準ライブラリの構文キャッシュを共有する。
生成Wasmの実行には毎回別のWorkerとインスタンスを作る。start関数も含め、無限ループはメイン画面を止めない。
コンパイルは30秒、実行は2秒でWorkerを終了する。「Stop」でも終了でき、次の操作で新しく作り直す。
これはCPU処理の停止手段であり、ブラウザープロセス全体のメモリ使用量の上限を保証するものではない。
ソースや最適化設定を変えると生成物を無効にし、古いコンパイル応答や実行結果は表示しない。

## 検査

```sh
just test-playground
```

Rustのアダプターテストはnative WAT・GC拡張・最適化on/offの生成物と診断を既存APIと照合する。
native版のCore＋JSPI生成との比較では所有権カーネルを実行するため、検査環境には`lean +v4.34.1`も必要。PagesのCIは既存の`ci/install-tools.py`で固定版を準備する。公開したWasmコンパイラの実行にLeanは不要。
`playground/samples.test.mjs`は機能の網羅と説明・期待結果のメタデータを検査する。
`playground/tests/samples.spec.mjs`は全サンプルをブラウザーでコンパイルし、実行値・不足するホストimport・意図した診断を照合する。
PlaywrightはWasmコンパイラの読み込み、GCを含むサンプル、実行・保存、診断と復帰、
無限ループのタイムアウト・停止、Leanの拒否、trap、不足するhost import、狭い画面を検査する。
イベントループのFuture・Stream、最適化off、Resultの失敗側、export名の変換、引数個数、待機の停止後の再実行も検査する。
固定7入力では、Wasm版コンパイラのWATが変更前のnative版の記録と一致することも確認する。
ビルド済みのときは`pnpm test:playground`でブラウザー検査だけを実行できる。
別のローカルサーバーが既定のポートを使っている場合は、`WLT_PLAYGROUND_TEST_PORT=41741 pnpm test:playground`で検査用ポートを変更できる。

サンプルの定義は`playground/samples/`に機能別に分け、`samples.mjs`で集める。
追加するときは`category`、`label`、`description`、`features`、`expectation`、単独で扱える`source`を指定する。
`expectation.kind`は`run`（`output`）、`compile`（ホストの`imports`と説明`note`）、
`diagnostic`（`code`、必要なら`message`）のいずれかにし、`just test-playground`で実際の挙動を確かめる。

スコープ付き子タスクを含むソースは、Core＋JSPI の生成物と専用 scheduler を使って実行します。WAT 表示はこの場合の実際の private ABI を含み、関数の選択肢には元ソースの export だけを出します。[spawn / join の契約](scoped-tasks.md) を参照してください。
