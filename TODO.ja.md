# Walz の実装計画

[English](TODO.md) | [日本語](TODO.ja.md)

小さい言語仕様、WAT の包含、Wasm GC、静的解析のしやすさを維持する。
チェックは実装と回帰検査の完了を意味し、設計だけでは付けない。

## バイナリ codec からのコア改善（1 → 2 → 3 → 4 → 5）

- [x] 1: 固定幅・明示 endian の bytes 読み取りと packed 配列への書き込み。型付き操作と `walz:core` の bytes/view API を追加。unsigned offset、16bit の符号拡張、整数／浮動小数点のビット保持、評価順、変更前の全範囲検査を定義。両 endian・非整列・view 内の境界・effect・opaque・両 JS backend を検査済み。
- [x] 2: immutable bytes-view の範囲を直接 UTF-8 検証し、payload をコピーせず string-view にする。明示的な実体化、元バッファの保持、全 backend での厳密な UTF-8 規約を記載・検査済み。
- [x] 3: サイズ／offset の加算・乗算・整列を検査する core helper。Wasm i32/i64 の wrapping を変更せず、失敗を明示し、暗黙の数値変換を増やさない。
- [x] 4: 検証済みの不変範囲を view・小さな共有値の関数のインライン化・各反復の unsigned ガード経由で引き継ぐ。独立した must 解析で証明した冗長な検査を除き、未確認の検査と Wasm GC 配列の境界検査は残す。
- [x] 5: 条件を満たす共有の tuple/Option/Result の格納用割り当てを、ローカルの match/drop・分岐・小さな関数のインライン化経由で除去。内部の戻り値にも適用し、payload の同一性・評価順・明示的な trap/例外・所有権の境界と公開 ABI を維持。
- [ ] 後続: 大きい関数・再帰・ループで持ち回る値の汎用 multi-value 特殊化。現在の consumer fusion は未確認・escape の格納表現を残し、コード複製量を制限する。
- [ ] 後続: 可変バッファを消費して immutable bytes/list にする freeze。内部 storage の排他所有を先に定義する。現在の Own は一意性を保証しない。コピー削減と保持される capacity の両方を測定。

構文の追加は小さく保ち、codec 固有命令より core API と型付き IR を優先する。ABI/runtime は [FlatBuffers の公開計画](walz/flatbuffers/TODO.md) を参照。

1 の仕様は[固定幅アクセス](docs/byte-access.md)に記載し、FlatBuffers の reader/builder でも使用する。`just test-byte-access` で再検査でき、Chromium は `runtime/e2e/byte-access.spec.mjs` に収録。workspace テスト、`just check`、両 JS backend、bytes/FlatBuffers の Chromium 8件が成功。

2 は `$string-view.from-bytes-view`、`$bytes-view.is-utf8` / `to-string`、`@bytes.is-utf8` / `@string.from-bytes` の範囲指定を追加。共通 validator は選択範囲だけを読み、不正 UTF-8 は none/0、範囲外は trap。検証成功後にコピーする。FlatBuffers も範囲付き decode を使用。[view の規約](docs/views.md#validate-a-byte-range-as-a-string-view)を参照。`just test-utf8-views` と `just test-views` で再検査できる。GC 同一性・解放、suspend/reject/cancel、4方式での厳密な decoder との比較、workspace テスト、`just check`、Node 23件、Chromium 20件が成功。

3 は `$i32.checked-add-u`、`$i32.checked-mul-u`、`$i32.checked-align-up-u` と i64 版の通常関数6つを追加。Option を返し、符号なしの最上位ビットと非ゼロの2の累乗の整列に対応する。不正な整列・overflow は none。[規約](docs/checked-integers.md)と[レイアウト例](examples/checked-layout.walz)を参照。FlatBuffers は要素ループ外でベクトルの byte 数を一度だけ検査し、アプリ側の上限を維持する。再実行は `just test-checked-integers`。Wasmtime の最適化 on/off、4方式と無限精度 BigInt の比較、workspace 681件、Node 11件、`just check`、対象を絞った Chromium 8件が成功。最初の全 fixture 起動は300秒でタイムアウトしたため、対象 fixture を独立して再ビルドした。

4 は、ローカル・ループ識別子を刷新する作業量制限付きインライン化と、不変なスナップショット・アクセスの正常終了・unsigned ガードを追う must 解析を追加。検証済みの部分範囲を折り返しなく合成し、ループの初期値は不変条件に使わない。証明した reader は helper 内の明示的な検査2つを省き、GC の `array.get_u` の検査と元の分岐条件の評価は維持する。診断情報は削除判断に使わない。signed/i64 推論、再帰・一般的な関数間契約、未対応の制御構造をまたぐ範囲は検査を残す。[規約](docs/range-optimization.md)と[走査例](examples/range-scan.walz)を参照。再実行は `just test-range-optimization`。最適化 on/off の raw Wasm 比較で null trap・ホスト副作用・例外・可変フィールドの再読込・複数のネストしたループ呼び出しを検査。workspace 686件が成功（既存の ignore 3件）、`just check`、`just proof-check`、Node 19件、現行ソースを再ビルドした範囲・FlatBuffers の Chromium 8件も成功。Lean の証明対象は算術規則で、処理系全体ではない。ベンチマーク上の速度向上は未計測。

5 は Wasm 関数のシグネチャを変えず、consumer fusion と共有値の関数のインライン化を使用。対象のローカル値を payload のローカル変数に置き換え、複数の match・未使用値・検査付き関数の入れ子・unit/null の区別・tuple/Result を返す処理にも対応。従来の構築直後の Result 最適化が `@default` の payload の副作用・trap まで落とす不具合も修正した。未知・escape・外部値の subtype 検査、構築側の try/label/loop のスコープ、同一性を持つ enum、owned/resource/borrow を扱う関数は規約を維持し、作業量制限で中止した変換は元に戻す。[規約](docs/value-optimization.md)と[パイプライン例](examples/value-pipeline.walz)を参照し、`just test-value-optimization` で再検査できる。workspace 692件（既存の ignore 3件）、`just check`、`just proof-check`、Node 35件、対象を絞った値・範囲・FlatBuffers の Chromium 12件が成功。raw Wasm の最適化 on/off、ホストの評価履歴、不正な外部値、4方式と BigInt の比較、payload の reject/cancel、ABI の後始末を検査。汎用 multi-value と速度の実測は後続で、変換の正しさは compiler TCB に属し、新しい Lean の証明対象ではない。

## Angle 型引数表記

- [x] 型・関数・heap 型の参照位置で `$Name<T, U>` を `@inst` の糖衣として受け付ける。入れ子の値型、汎用クロージャ、名前付き構築、マクロの型引数、具体化した export に対応。参照 wrapper の明示、既存の検査と具体化の共有、元の位置、WAT のリテラル名、演算子を維持し、formatter・Watz 表示・LSP の回復・Playground サンプルにも対応。

[規則](docs/generics.md#angle-application-shorthand)と[実行例](examples/angle-applications.walz)を参照し、`just test-angle-applications` で再検査できる。最適化 on/off で明示的な `@inst` と出力 Wasm が一致することを検査。名前空間・import の別名、WAT の通常名・引用名・suffix を含む名前、不正／深い適用、型制約、エラー位置、未完の編集バッファにも対応。workspace 701件（既存の ignore 3件）、`just check`、`just proof-check`、文書検査、Node 5件（4方式とサンプルの網羅性）、新しくビルドした Wasm コンパイラを使う独立した Chromium Playground 2件が成功。日英チートシート・仕様と Walz skill の参照を更新し、既存の Playground プロセスは維持した。

## ユーザー定義構文（1 → 2 → 3 → 4）

- [x] 1: 構文ノードの識別・展開コンテキスト・入力と出力の対応を保持。複製・削除・定義元を追跡。
- [x] 2: `@macro` の宣言的・衛生的な式マクロ、明示的な `@use` / `@export`、繰り返し、展開量の上限。
- [x] 3: LSP の署名・定義・補完・展開表示と診断。消えた引数や生成した一時変数の型を原文へ誤表示しない。
- [x] 4: `bind` による呼び出し側の束縛と本体スコープの宣言・整合検査。エラー編集中にも宣言情報を利用。

`infer` で通常の推論型を取得し、`when` で型・既存の型クラスによるルール選択を行う。生成した型適用も既存の単相化へ戻し、型・effect・所有権・race の検査を省略しない。[規約](docs/user-macros.md)、[実行例](examples/user-macros/main.walz)。

## マクロの実用性から追加した機能

- [x] 通常の汎用関数の型引数を呼び出し境界で推論。曖昧さ・制約・effect・所有権と同型再帰を検査。
- [x] `walz-type-info-v1` の読み取り専用型情報と構造 selector。opaque な内部型を公開しない。
- [x] マクロ／手書き展開／独立モデルの値・副作用・例外・所有権の差分 fuzz、同じ失敗分類での縮小と replay。
- [x] JSON／LSP と `macro-expand --text` で入力の複製・破棄を表示。構文出現数と実行回数を区別。

## マクロ評価で残った4課題

- [x] 1: 展開後は該当関数だけを再検査。型環境が変わる具体化は全体を再検査し、最終的な全体の契約検査を維持。既存上限で256/512関数を検査。
- [x] 2: 呼び出し先と一致する明示的な束縛インターフェースの委譲。初期値への漏れと曖昧なルールを拒否。
- [x] 3: 反復identのbind、letグループの展開順、空列・重複・初期値のcallerスコープと多段委譲。
- [x] 4: 未完のバッファでも現在のimport・未保存のマクロ宣言を回復。型・展開は未検査のまま、元ファイルの定義位置と補完を提供。

[同じ評価入力による追試](evals/reports/20261109-macro-usability-v1-r2/REPORT.md)：必須20検査と前回の未達4課題が成功。

## マクロ評価 v2 の改善

- [x] 同一関数の検査を式から再開。推論の仮ローカルを持ち越さず、具体化時の全体再検査と最終契約検査を維持。単一関数512展開と深い展開の診断を回帰検査。
- [x] 末尾以外の反復、body 全体の転送、初期化式の caller スコープを診断の help で説明。
- [ ] trap・例外・所有権を維持できるスカラー式／ローカルの削除を検証。
- [ ] 再帰的な Result 束縛マクロを別課題で評価し、追加構文の必要性を判断。

[同じ入力による比較](evals/reports/20261109-macro-authoring-v2-r3/REPORT.md)：必須11検査が成功、単一関数512展開が成功。残る合成課題3件は現行仕様の制約として維持。

## 非同期機能を実用へ接続する（1 → 2 → 3 → 4）

- [x] test / bench の Core＋JSPI、明示 host と binding、ケース単位の解放と期限。
- [x] WIT の共有値型と同期 world export adapter。native async / resource の対応範囲は下記を参照。
- [x] 明示的な Stream batch read、部分受理を返す sendMany、ファイルの chunk 転送と比較 bench。
- [x] 公開呼出しとホスト側スコープの期限・取消し、所有する timer future。Walz 内の子取消し・join の期限も追加。

[バッチ転送](docs/stream-batches.md)、[スコープと期限](docs/scoped-tasks.md)。

## 今回の追加（1 → 3 → 5 → 4）

- [x] 1: 型グラフ・ABI layout/flatten/schema のメモ化、サイズ制限と DAG 回帰検査。
- [x] 3: Walz の子タスク単位の取消しと join-timeout。両 JS backend とブラウザで解放を検査。
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

- [x] 1: `@for-each`。共有可能な非unit要素のlistを、単一名・tuple・フィールドpatternで走査。
- [x] 2: `@let {:field $binding ...}`。non-nullの共有構造体から必要なフィールドを一度読み、ローカルに束縛。
- [x] 3: `@struct.update`。対象・旧値・更新式を一度ずつ評価する、通常の可変フィールド更新。

[規約](docs/high-level-forms.md) と [実行例](examples/high-level.walz)。既存IRへ展開し、型・effect・所有権・opaqueを検査。LSP、最適化の有無、Core / Component / Chromiumの実行と中断・例外経路を検査する。

## CLI から実用アプリケーションまで（2 → 1 → 3 → 4 → 5）

Rust 1.99 を処理系と CI の基準にする。

- [x] 2: `walz run` のエントリー・引数・標準入出力・終了コード・明示ホスト capability。割り込みの解放完了も待つ。
- [x] 1: `walz build --target core-js` に必要なランタイムだけを同梱し、チェックアウト不要の JS 成果物を生成する。
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
6. [x] JSON 値と parser を Walz ライブラリで実装する。配列・null・数値の字句と Unicode escape を扱い、重複を確認してからジェネリクスを判断する。

1 の API と境界は [walz/core/README.md](walz/core/README.md)、変更前後の出力照合・測定値・生成サイズは [実装記録](docs/stdlib/20261105.md) に保存した。
JSON 値・parser・配列・null・小数/指数の字句を扱う API を追加した。数値は正確な text として保持する。binary float の変換は後続。詳細は [JSON 値](docs/json-values.md)。

## 型付きコアの小さな拡張（1 → 2 → 3 → 4 → 5）

- [x] 名前付きstruct構築。`@struct.new`、独立したfield-map・期待型リテラルの糖衣、ソース順一回評価、opaque・所有権・effectの既存検査を維持する。[規約](docs/struct-construction.md)。
- [x] ローカル参照と期待型付き数値の省略形、letの`(type T)`注釈。曖昧な幅・暗黙のmove/変換は認めず、単相化・静的解析・両JS ABIで確認する。
- [x] 基本型向けのAdd/Eq/Ord制約と記号演算子。単相化時に固定インスタンスを検査し、辞書を作らず既存のWasm命令へ解決する。[規約](docs/typeclasses.md)。
- [x] Logical/Bitwise/Shift制約。短絡する`&&`/`||`、`&`/`|`/`^`、`<<`/`>>`/`>>>`。右辺の静的検査・所有権・待機とWasmのシフト規則を維持する。
- [x] Sub/Mul/Div制約と`- * /`。Wasmの折返し・符号付き除算のtrap・float除算を保ち、json-valueを省略記法で整理する。

- [x] 1: 標準スカラーの変換・飽和変換・丸め・sqrt・min/max・reinterpret・回転・符号拡張。署名をWasm validatorと照合する。
- [x] 2: `@result.do`。既存のresult.letへ展開し、左から右の一度だけの評価とerrの短絡を保つ。
- [x] 3: スカラーnewtypeと明示wrap/unwrap。nominalな検査とopaqueを保ち、実行用表現では消去する。
- [x] 4: 型付きreturn・名前付きblock/br/br_if。所有権とescapeを追い、抜けるusingだけを逆順にcloseする。
- [x] 5: 所有値と例外の併用。引数評価途中の所有slot、catch、反復、未処理例外の経路をRustとLeanで検査する。

仕様と限界は [数値・Result・newtype・制御](docs/core-expressions.md)。`just test-core-expressions`、Nodeの両ABI、Chromiumで検査する。

## 開発ツールと標準コレクション

- [x] `check --json` で既存の診断契約と終了コードを機械処理可能にする。
- [x] `type-at` で元ファイルの行・Unicode列を問い合わせ、型注釈・型alias・具体化位置を返す。詳細は [外部ツール向けの検査](docs/tooling.md)。
- [x] textキーの `dict<T>` と、不変listを作る `list-builder<T>` を通常のWALZライブラリとして提供する。共有可能・非unitの制約、opaque、effectを検査する。
- [x] 衝突・拡張・更新・削除・再利用・参照同一性・不変スナップショットをWasmtimeで検査し、Core＋JSPI / Componentを独立したMapと照合する。詳細は [コレクション](docs/collections.md)。

## リポジトリの独立

- [x] Wasmtime の作業ディレクトリから独立させ、言語・crate・CLI・拡張子を Walz / walz / .walz に改名する。
- [x] `mizchi/walz-lang` を公開し、独立した環境での検査結果を記録する。

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

- [x] WIT付きの閉じたComponent `.wasm`の同期・共有値exportを`@use`する。型・effect検査、選択・別名・再export、同一インスタンス、依存全体のDCEと静的埋込み。
- [ ] Component依存のasync／Future／Stream／resource境界。取消し例外・join・端点の寿命の契約を定めてから受理する。
- [ ] Component依存のホストcapabilityの明示的な接続と、Core／Core＋JSPIでの呼出し。現状はComponent出力のみ。

- [x] namespace/public/use と名前空間、相対 include、循環検出、単一 Wasm への静的リンク。
- [x] 非公開関数・型・tag と、公開する名前の検査。
- [x] 公開する型の opaque 宣言。別名と推論された参照にも適用。
- [x] ファイル単位の named import/export と再公開。同じ canonical path の定義・型を共有し、選んだ名前だけを束縛する。
- [ ] フィールド単位の可視性。
- [x] bytes/text の標準ライブラリと、ログ集計の可変な内部表現の隠蔽。

## 5. 利用者定義のジェネリクス

- [x] 型・具体的なクロージャ型を引数とする単相化。明示した type.params と適用形を関数・型・alias・クロージャで共有。
- [x] 具体化後の型と呼び出し先を閉じたままにする。型・effect・所有権・opaque を DCE 前に検査。
- [x] map/fold などを重複せず記述する。walz/core/list.walz と examples/generic-lists.walz。詳細は [単相化](docs/generics.md)。
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
- [x] Walz 内の子だけの取消し・期限。親と兄弟を保ち、子孫と disposal の完了を待つ。
- [ ] select と任意スコープの取消しトークン。
- [x] 共有 payload の future/stream を標準の native async ABI に接続。Walz SDK の子 scheduler は別境界。

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
- [x] Node／ブラウザの汎用WorkerHostへキュー・個別取消し・join・終了を分離。索引layoutはdriverに置く。別の非同期WALZをCore＋JSPIとComponentで実行。
- [x] 公開前コピー・非公開のshared memory・全reader終了待ち。取消しの個別bit、終了失敗の再試行、Chromiumを検査。
- [ ] 小さい索引向けの共有memory capacity削減。現在16MiB固定で、byteLengthは使用サイズ。
- [ ] 索引構築・Worker起動を含むcold-startの分布とp95、実アプリでの損益分岐を計測する。

[Workerホスト](docs/worker-host.md)、[readonly領域](docs/readonly-regions.md)、[索引と計測](docs/succinct.md)。任意のJS driverやホスト全体のrace-free、静的Builder freeze、GC参照のスレッド間転送は保証に含めない。
