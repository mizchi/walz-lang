# WLT の実装計画

小さい言語仕様、WAT の包含、Wasm GC、静的解析のしやすさを維持する。
チェックは実装と回帰検査の完了を意味し、設計だけでは付けない。

## 非同期機能を実用へ接続する（1 → 2 → 3 → 4）

- [x] test / bench の Core＋JSPI、明示 host と binding、ケース単位の解放と期限。
- [x] WIT の共有値型と同期 world export adapter。native async / resource の対応範囲は下記を参照。
- [x] 明示的な Stream batch read、部分受理を返す sendMany、ファイルの chunk 転送と比較 bench。
- [x] 公開呼出しとホスト側スコープの期限・取消し、所有する timer future。WLT 内の子取消し・join の期限も追加。

[バッチ転送](docs/stream-batches.md)、[スコープと期限](docs/scoped-tasks.md)。

## 今回の追加（1 → 3 → 5 → 4）

- [x] 1: 型グラフ・ABI layout/flatten/schema のメモ化、サイズ制限と DAG 回帰検査。
- [x] 3: WLT の子タスク単位の取消しと join-timeout。両 JS backend とブラウザで解放を検査。
- [x] 5: stdio LSP の diagnostics / hover / definition。UTF-16、未保存 import、cache 上限、request cancellation。
- [x] 4: native WIT async worlds と imported resource の own/borrow。標準 Canonical ABI、raw jco と Wasmtime 50 で検査。

[LSP](docs/lsp.md)、[native worlds](docs/native-worlds.md) に実行条件と未対応範囲を記載する。HTTP や WASI ネットワーク binding は別工程。

## 使い勝手の修正（1 → 2 → 3 → 4 → 5）

- [x] 1: 通常のJS/Component ABIからtest/benchの生成exportを除外し、全ソース検査と各runnerの動作を維持。
- [x] 2: 単相化した型・関数の診断を、型引数とスコープ内の別名で表示。診断表示の展開も制限。
- [x] 3: LSPで現在の宣言syntaxを回復し、未検査表示を区別。未入力の括弧、定義移動、lexical shadowing、core importのcompletion。
- [x] 4: 個別取消し・native own/borrow・backend別stdoutの説明を同期。
- [x] 5: 期待型が確定したstringリテラルと、shared/nonunitの`$option.ok-or` / `$result.map-err`。評価順とGC同一性を検査。

[修正後の評価](evals/reports/20261108-usability-fixes-v2/REPORT.md) に検査結果と制限を記録した。

## 高水準の糖衣（1 → 2 → 3）

- [x] 1: `%for-each`。共有可能な非unit要素のlistを、単一名・tuple・フィールドpatternで走査。
- [x] 2: `%let {:field $binding ...}`。non-nullの共有構造体から必要なフィールドを一度読み、ローカルに束縛。
- [x] 3: `%struct.update`。対象・旧値・更新式を一度ずつ評価する、通常の可変フィールド更新。

[規約](docs/high-level-forms.md) と [実行例](examples/high-level.wlt)。既存IRへ展開し、型・effect・所有権・opaqueを検査。LSP、最適化の有無、Core / Component / Chromiumの実行と中断・例外経路を検査する。

## CLI から実用アプリケーションまで（2 → 1 → 3 → 4 → 5）

Rust 1.99 を処理系と CI の基準にする。

- [x] 2: `wlt run` のエントリー・引数・標準入出力・終了コード・明示ホスト capability。割り込みの解放完了も待つ。
- [x] 1: `wlt build --target core-js` に必要なランタイムだけを同梱し、チェックアウト不要の JS 成果物を生成する。
- [x] 3: WIT の record / enum / flags / unsigned・world 選択・型付き export 雛形、および WASI 0.3.0 のファイル値 adapter。
- [x] 4: Wasm validator の型を共有し、数値・SIMD のメモリ命令と型付きマクロを同じ関数で扱う。未知の命令と所有権の境界は閉じる。
- [x] 5: 共有 payload の Future / Stream writer、backpressure・close・取消し、スコープ付き spawn / join。両 JS backend と Playground で実行する。

[CLI](docs/cli.md)、[WIT](docs/wit-bindings.md)、[WASI 値 adapter](docs/wasi-files.md)、[validator](docs/wat-validator.md)、[writer / 子タスク](docs/scoped-tasks.md) に対応範囲と制限を記載した。

## 直近の優先順位

複雑なアプリケーションで確認した不足を、次の順に進める。

1. [x] bytes/text の検索・分割・検査付き整数解析と text/JSON builder を標準ライブラリにまとめる。ログ集計・在庫台帳・workflow で再利用し、出力と計測値を照合する。
2. [x] マクロ版と明示版、Wasmtime と Node の差分検査を増やす。不正ソースと取消しのタイミングを fuzz し、失敗入力の縮小と回帰 corpus の保存まで行う。
3. [x] 既存の値型を対象に WIT binding を生成する。effect/capability は WIT から推測せず、別の信頼する policy で指定する。
4. [x] 型付きの外部 resource handle と明示 close を追加し、正常・Result の err・例外・取消しで終了処理を検査する。現在の own box は OS 資源の destructor を持たない。
5. [x] 小さな型付き IR 最適化として、生成直後に match する Result/Pair を簡約する。評価順・例外・所有権を保つ検査を用意し、証明済みの範囲と未証明の範囲を分ける。
6. [x] JSON 値と parser を WLT ライブラリで実装する。配列・null・数値の字句と Unicode escape を扱い、重複を確認してからジェネリクスを判断する。

1 の API と境界は [stdlib/README.md](stdlib/README.md)、変更前後の出力照合・測定値・生成サイズは [実装記録](docs/stdlib/20261105.md) に保存した。
JSON 値・parser・配列・null・小数/指数の字句を扱う API を追加した。数値は正確な text として保持する。binary float の変換は後続。詳細は [JSON 値](docs/json-values.md)。

## 型付きコアの小さな拡張（1 → 2 → 3 → 4 → 5）

- [x] 名前付きstruct構築。`%struct.new`、独立したfield-map・期待型リテラルの糖衣、ソース順一回評価、opaque・所有権・effectの既存検査を維持する。[規約](docs/struct-construction.md)。
- [x] ローカル参照と期待型付き数値の省略形、letの`(type T)`注釈。曖昧な幅・暗黙のmove/変換は認めず、単相化・静的解析・両JS ABIで確認する。
- [x] 基本型向けのAdd/Eq/Ord制約と記号演算子。単相化時に固定インスタンスを検査し、辞書を作らず既存のWasm命令へ解決する。[規約](docs/typeclasses.md)。
- [x] Logical/Bitwise/Shift制約。短絡する`&&`/`||`、`&`/`|`/`^`、`<<`/`>>`/`>>>`。右辺の静的検査・所有権・待機とWasmのシフト規則を維持する。
- [x] Sub/Mul/Div制約と`- * /`。Wasmの折返し・符号付き除算のtrap・float除算を保ち、json-valueを省略記法で整理する。

- [x] 1: 標準スカラーの変換・飽和変換・丸め・sqrt・min/max・reinterpret・回転・符号拡張。署名をWasm validatorと照合する。
- [x] 2: `%result.do`。既存のresult.letへ展開し、左から右の一度だけの評価とerrの短絡を保つ。
- [x] 3: スカラーnewtypeと明示wrap/unwrap。nominalな検査とopaqueを保ち、実行用表現では消去する。
- [x] 4: 型付きreturn・名前付きblock/br/br_if。所有権とescapeを追い、抜けるusingだけを逆順にcloseする。
- [x] 5: 所有値と例外の併用。引数評価途中の所有slot、catch、反復、未処理例外の経路をRustとLeanで検査する。

仕様と限界は [数値・Result・newtype・制御](docs/core-expressions.md)。`just test-core-expressions`、Nodeの両ABI、Chromiumで検査する。

## 開発ツールと標準コレクション

- [x] `check --json` で既存の診断契約と終了コードを機械処理可能にする。
- [x] `type-at` で元ファイルの行・Unicode列を問い合わせ、型注釈・型alias・具体化位置を返す。詳細は [外部ツール向けの検査](docs/tooling.md)。
- [x] textキーの `dict<T>` と、不変listを作る `list-builder<T>` を通常のWLTライブラリとして提供する。共有可能・非unitの制約、opaque、effectを検査する。
- [x] 衝突・拡張・更新・削除・再利用・参照同一性・不変スナップショットをWasmtimeで検査し、Core＋JSPI / Componentを独立したMapと照合する。詳細は [コレクション](docs/collections.md)。

## リポジトリの独立

- [x] Wasmtime の作業ディレクトリから独立させ、言語・crate・CLI・拡張子を WLT / wlt / .wlt に改名する。
- [x] `mizchi/wlt-lang` を公開し、独立した環境での検査結果を記録する。

## 1. 文字列・bytes・list の境界表現

- [x] 不変な bytes/text の GC 表現と WIT list<u8>/string の Canonical ABI adapter。
- [x] 共有要素の不変 list<T> と ABI adapter。所有要素と borrow は禁止。詳細は [list](docs/lists.md)。
- [x] UTF-8 の厳密な検証、バイト長・符号なし添字・開始位置と長さによるスライス。
- [x] 検査付き bytes.from-array と、連結・整数変換・JSON string の標準ライブラリ初版。
- [x] bytes/text の検索・最初の区切りでの分割と、断片をためる text/JSON object builder。
- [x] 不変bytes/stringのゼロコピービュー。相対範囲・UTF-8境界・opaqueとeffectを検査し、GCでの共有・回収と両ABIを確認する。標準型名は`$string`に統一する。[規約](docs/views.md)。
- [ ] Unicode の文字数・書記素・正規化などの操作。

## 2. 汎用 WIT binding と実用 I/O

- [x] 対応する WIT 値型から import と export 実装雛形の型を生成する。
- [x] 同期の共有値 world の native export instance adapter。元の interface 名と型・関数署名を検査する。
- [x] native world の共有 payload async endpoint と imported resource own/borrow ABI。所有要素の list・record と full WASI worlds は対象外。
- [x] 固定した text I/O の型・host effect・必須 capability と manifest、明示的な host 接続。
- [x] 対応する WIT 値型の freestanding import と明示的な effect/capability policy。対応範囲は [WIT binding](docs/wit-bindings.md) を参照。
- [x] Node の標準入出力・ファイルを、値だけを渡す text I/O capability に接続。
- [x] WASI 0.3.0 に固定したファイル値 adapter。descriptor はホスト所有、名前付きの明示 grant を使う。
- [ ] WASI の native resource world とネットワーク binding。
- [x] UTF-8 入力の空行をフィルタして出力し、途中取消しでも解放する CLI。

## 3. 外部 resource と終了処理

- [x] 型付き resource handle と同期 destructor。既存の own box とは区別する。
- [x] 明示 close と using。正常・例外・取消しの全終了経路を検査する。
- [x] 所有値と try/throw の併用。正常・例外・分岐経路と評価途中の所有値を検査し、Leanの制御フロー証明書で照合する。
- [ ] 所有 payload の Future/Stream 配送。
- [x] 必要な ABI 境界に限定した borrow。一般的な lifetime 機構は必要性を見て判断する。

## 4. モジュールと可視性

- [x] namespace/public/use と名前空間、相対 include、循環検出、単一 Wasm への静的リンク。
- [x] 非公開関数・型・tag と、公開する名前の検査。
- [x] 公開する型の opaque 宣言。別名と推論された参照にも適用。
- [x] ファイル単位の named import/export と再公開。同じ canonical path の定義・型を共有し、選んだ名前だけを束縛する。
- [ ] フィールド単位の可視性。
- [x] bytes/text の標準ライブラリと、ログ集計の可変な内部表現の隠蔽。

## 5. 利用者定義のジェネリクス

- [x] 型・具体的なクロージャ型を引数とする単相化。明示した type.params と適用形を関数・型・alias・クロージャで共有。
- [x] 具体化後の型と呼び出し先を閉じたままにする。型・effect・所有権・opaque を DCE 前に検査。
- [x] map/fold などを重複せず記述する。stdlib/list.wlt と examples/generic-lists.wlt。詳細は [単相化](docs/generics.md)。
- [x] where による shared/nonunit/callable と effect 上限。具体化時に型・署名・推論 effect を検査し、制約を実行用コードへ残さない。

## 6. 解析精度と契約の拡張（6.1–6.3 の初版を実装）

- [x] 6.1 引数・捕捉・割当て箇所・不明を区別する読み書きの対象。
- [x] 6.2 別名・格納先・呼び出し・返却を追う escape 解析。
- [x] 6.3 分岐で得た non-null・i32 の範囲・配列境界の事実を式の位置とともに保持する。
- [ ] 6.4 明示的なループ不変条件・減少量と、入口／反復での証明義務。
- [ ] 6.5 GC・配列・再帰を含む事前条件／事後条件と law の拡張。
- [ ] 6.6 解析の根拠を使う最適化パスと意味保存の検査。
- [x] 到達しない関数・型・tag・ホスト import の除去。型・effect・所有権の全ソース検査を維持し、Core / Component / profiler の ID と ABI をそろえる。
- [x] Wasmtime guest profiler と Core＋JSPI の再現可能な比較計測、ABI 割当てと UTF-8 検証のチューニング。

既存の effect 上限・所有権検査は維持する。may 情報を must な非別名性や純粋性へ読み替えない。
未知のコード、再帰、ループ、例外、中断を、未検査の仮定で処理しない。
解析の改善と、契約モデル／コンパイラの健全性の証明を分けて記録する。

6.1–6.3 は `analyze` の memory/facts と Rust API に実装した。引数間の非別名性は仮定せず、
割当ては箇所単位で併合し、アクセスパスは深さ3で不明へ広げる。
数値範囲は i32 の符号なしビット区間を対象とし、ループの持ち回り値は型の上限へ広げる。
i64 の区間、可変フィールドの値の精密化、関数間の範囲契約は後続。
既存の effect 契約や Wasm 出力は変更せず、得た情報による命令除去は6.6で扱う。

## 7. writer と言語内タスク

- [x] JS 向け Core＋JSPI 出力。使用する型の codec を生成し、既存の取消しと Stream の終了処理を共有する。
- [x] 共有 payload の pending Future/Stream writer と、既存 resource を使う所有権の protocol。
- [x] 一要素単位の受理・終了、backpressure、close と取消し。
- [x] スコープ付き spawn/join/drop。明示捕捉、GC 同一性、兄弟の失敗・親の取消しと解放を検査する。
- [x] Stream のバッチ転送と部分進捗、公開呼出しとホスト側スコープの期限・取消し。
- [x] WLT 内の子だけの取消し・期限。親と兄弟を保ち、子孫と disposal の完了を待つ。
- [ ] select と任意スコープの取消しトークン。
- [x] 共有 payload の future/stream を標準の native async ABI に接続。WLT SDK の子 scheduler は別境界。

## 8. 任意の WAT 命令とマクロの混在

- [x] `v128` と通常 SIMD の非メモリ命令を型付きコアで検査。GC格納・捕捉・単相化、Core / Component / Chromium で確認。ABI 境界はスカラー等へ明示変換。[SIMD](docs/simd.md)。
- [x] jsimdを参考に4種類のバルクカーネルを実装し、保持したGC値とコピー込みの双方でJS・参照Wasmと比較。[測定結果](docs/simd-kernels.md)。
- [x] SIMDの実測で見つかったCore＋JSPIの数値list ABIを、対応するTypedArrayの一括コピーで改善。値検査・解放とArrayの経路を維持し、同一Wasmで変更前後を交互に測定。
- [ ] pack前の中間GCコピーを減らし、pack済みデータの再利用を増やす。型検査・値検査・例外時の解放を維持し、保持したデータと単発呼び出しを分けて再測定する。
- [x] 数値・SIMD の線形メモリ命令と型付きマクロを混在させる。
- [x] その命令の型と結果を Wasm validator の operand stack と共有する。
- [x] 閉じた命令一覧、保守的な effect / escape、数値だけの境界で所有権の迂回を拒否する。
- [ ] 任意の stack 区間・多値・table/ref 命令への一般化。未知の opcode の自動受理はしない。

bytes/text とモジュールは Rust/Node/Chromium で検査する。構文と ABI の詳細は SPEC.md を参照。
UTF-8 validator、ファイル展開、名前解決と adapter は TCB に含み、Lean による意味保存の証明はまだない。

## ログ集計からの改善（1 → 2 → 3 → 4 → 6）

- [x] 1: GC 配列のコピーと文字列ライブラリ。別名、UTF-8、整数境界を検査。
- [x] 2: result.let / stream.fold-result。展開後の型・所有権・effect 検査、EOF・err・取消し時の解放。
- [x] 3: opaque 型とレベル enum。内部からだけ変更できる集計状態。
- [x] 4: コメントを保持する fmt / fmt-check と、開始括弧位置・アプリケーションエラーの診断。
- [x] 6: 行長・不正率・チャンクサイズを変えた交互比較。JSON 1行出力、行 enum の統合、集計状態の直接参照。

この順序の5（単相化ジェネリクス）は明示的な型引数と具体化後の検査を実装した。6.6 は共有 Result/Pair の直後の match の簡約を実装し、一般の解析依存の最適化は後続。

## 改善後の実用例での確認

- [x] Unicode の品目名を扱う在庫台帳。具体的な GC ハッシュ表と動的 JSON Lines。
- [x] 手書き版とマクロ版の結果・I/O 失敗・取消し後の解放の照合、Core / Component / Chromium での実行。
- [x] main の記述量、展開後 IR、生成サイズ、交互実行時間の記録。
- [x] bytes/text の検索・分割・整数解析のライブラリ化。
- [x] 直後に match する共有 Result/Pair の簡約。意味保存の検査を伴う6.6の課題として扱う。

## 共有メモリの検証

- [x] 固定atomicセルの検証モード、ホスト契約、独立Lean traceカーネル。
- [x] rawアドレス・部分重複・非atomic命令の拒否と、load/storeの論理的な更新喪失の対照実験。
- [x] 公開後不変のreadonly領域、論理範囲を検査するload、私有memory併用と独立Lean traceカーネル。ホスト初期化・不変性は前提として明示。
- [ ] Owned<R>による排他領域の分割・Worker間のmoveとjoin。
- [ ] ロックやチャネルのプロトコル不変条件・線形化可能性。データ競合排除と別に扱う。

## 簡潔索引と汎用マルチスレッドホスト

- [x] 不変BitVectorのrank/selectとu8 Wavelet MatrixのrangeFreq/quantile。空・境界・欠落を独立oracleと同方式JSで照合。
- [x] スカラー／SIMDの交互比較で、単独rankはスカラー・Wavelet内部はSIMDを選ぶ。1/2/4 Worker、実際のjsimd公開API、通信込みのバッチ計測を新しいrunとして保存。
- [x] Node／ブラウザの汎用WorkerHostへキュー・個別取消し・join・終了を分離。索引layoutはdriverに置く。別の非同期WLTをCore＋JSPIとComponentで実行。
- [x] 公開前コピー・非公開のshared memory・全reader終了待ち。取消しの個別bit、終了失敗の再試行、Chromiumを検査。
- [ ] 小さい索引向けの共有memory capacity削減。現在16MiB固定で、byteLengthは使用サイズ。
- [ ] 索引構築・Worker起動を含むcold-startの分布とp95、実アプリでの損益分岐を計測する。

[Workerホスト](docs/worker-host.md)、[readonly領域](docs/readonly-regions.md)、[索引と計測](docs/succinct.md)。任意のJS driverやホスト全体のrace-free、静的Builder freeze、GC参照のスレッド間転送は保証に含めない。
