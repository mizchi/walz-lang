# WLTの機能一覧

WLTはWATのS式を保ち、Wasm GCを前提に型付きの高水準構文を追加する言語。
主な方針は、小さい構文・明示的な境界・静的解析のしやすさ。
このページは現在の実装を探すための一覧で、各機能の細かい規約はリンク先を正とする。
導入は[Getting started](getting-started.md)、書き方は[言語ガイド](language-guide.md)を参照。

## 構文・型・データ

| 機能 | 現在できること | 規約・制限 | 詳細 |
| --- | --- | --- | --- |
| 標準WAT | 標準命令・宣言をWasm validatorで検査してコンパイル | 任意のWATへWLTの所有権やeffect保証を付けるものではない | [構文](syntax.md)・[validator](wat-validator.md) |
| 型付きの関数 | 引数・結果型、数値命令、call、if、return、ラベル付き分岐 | 型付きコアの結果はunitまたは1値。標準WATの多値結果とは区別 | [基本の書き方](language-guide.md)・[式](core-expressions.md) |
| ローカルと省略形 | 不変の`%let`、複数束縛、`$x`、期待型付きの数値・文字列 | 数値幅の既定値・暗黙の数値変換はない | [束縛](standard.md)・[値操作](option-result.md) |
| 制御構造 | 状態付き`%loop`/`%recur`、`%for`、`%for-each`、Wasm例外の`%try` | recurは最内側ループの末尾。for-eachは共有list・unit本体 | [チートシート](../CHEATSHEET.md)・[走査](high-level-forms.md) |
| GC構造体・配列 | nominalな参照、再帰型、明示的な`mut`、nullable参照 | GCはエージェントごと。共有参照は参照先の不変性を意味しない | [チートシート](../CHEATSHEET.md) |
| 名前付き構築・分解 | `%struct.new`、`%$Type::{...}`、field pattern、field更新 | 全フィールド必須。型のないobject/map literalやspreadはない | [構築](struct-construction.md)・[更新](high-level-forms.md) |
| enum/match | payload付きvariant、再帰enum、網羅性・重複検査 | 自動の型絞り込みや任意の入れ子patternとは区別 | [言語ガイド](language-guide.md)・[チートシート](../CHEATSHEET.md) |
| 値の抽象 | `wlt:core`のoption・result・tuple・list・string・bytes | 型を明示して取り込む。list要素などの共有・非unit制約がある | [コア型](standard.md)・[list](lists.md) |
| ジェネリクス | 関数・型・alias・closureの単相化、通常callの型引数推論 | 曖昧なら`%inst`を指定。多相再帰や未適用テンプレート全体の証明はしない | [ジェネリクス](generics.md) |
| 型制約・演算子 | shared/nonunit/callable等の制約、Add/Mul/Eq/Ord等の組み込みクラス | 利用者定義class/implや動的な辞書渡しはない。float比較はWasm命令を使う | [制約](generics.md)・[演算子](typeclasses.md) |
| 型alias・newtype | 汎用aliasと、スカラーを区別するnominal newtype | newtypeは明示wrap/unwrap。基底型のクラスを自動継承しない | [式とnewtype](core-expressions.md) |
| クロージャ | 型・捕捉名・捕捉型を宣言し、構築時に値を渡す。汎用closureも使える | 暗黙のcaptureはない。所有captureのclosureは一度だけ呼べる | [言語ガイド](language-guide.md)・[例](../examples/closure.wlt) |
| マクロ | 衛生的な式マクロ、反復、束縛scope、推論型の問い合わせ | リーダーや標準WATを変更しない。生成式も通常の検査を通る | [マクロ](user-macros.md) |

## コアライブラリと計算処理

`wlt:core`から使う型・関数を列挙する。OSサービスを暗黙に取り込むモジュールではない。

| 機能 | 現在できること | 規約・制限 | 詳細 |
| --- | --- | --- | --- |
| string・bytes | UTF-8文字列、不変byte列、検索・分割・整数解析、builder | byte位置と文字位置を区別。全般的なUnicode正規化は提供しない | [生成API一覧](stdlib-api.md)・[標準ライブラリ](../stdlib/README.md) |
| list・dict・builder | 不変list、map/fold、stringキーの可変dict、list builder | 共有可能な要素が対象。dictは可変GC参照で、更新は別名からも見える | [list](lists.md)・[collections](collections.md) |
| ゼロコピーview | bytes-view・string-view・array-viewで元データの範囲を参照 | viewも小さなGC値を確保。小さいviewが元バッファ全体を保持し得る | [views](views.md) |
| JSON | 値の構築・走査・parse・文字列化をWLTで実装 | 数値の字句を保持。JSON数値とbinary floatの変換範囲はAPIを確認 | [JSON](json-values.md)・[parser計測](json-parser-bench.md) |
| SIMD | v128、Wasm SIMD命令、カーネル、数値listのABI一括転送 | JSへv128を直接渡すABIではない。数値listや線形メモリを使う | [SIMD](simd.md)・[カーネル](simd-kernels.md) |
| 簡潔データ構造 | bitset・rank/select・索引、常駐Wasm、readonly共有payload | ホストが配置・公開・寿命の契約を守る必要がある | [succinct](succinct.md) |

正確な引数・結果・effectは、ソースから生成する[標準API一覧](stdlib-api.md)を参照。

## 静的解析・所有権・証明

| 機能 | 現在できること | 保証の境界 | 詳細 |
| --- | --- | --- | --- |
| 型とeffect | 型照合、reads/writes/allocates/suspends/host/throwsの上限検査 | pureでもtrap・物理的割当て・非停止はあり得る | [解析](analysis.md) |
| 所有値・外部resource | 明示move、消費義務、例外・return経路の検査、SDKでの取消し時の終了処理 | GC参照一般の排他的所有やRust相当のlifetimeではない | [所有権](verification.md)・[resource](resources.md) |
| opaque・モジュール境界 | 表現を隠した型、明示した公開名・別名、静的リンクとDCE | 未使用の通常関数も除去前に検査。循環・wildcard・WLT動的importはない | [静的モジュール](static-modules.md) |
| 読み取り専用解析 | call graph、参照の由来・escape、i32範囲、配列長などを報告 | unknownやopaqueの範囲を残す。解析結果だけで検査を削除しない | [解析](analysis.md) |
| law・独立した契約検査 | 整数law、所有権証明書、制限したrace-free契約をLeanで検査 | コンパイラ・VM・ホスト全体の意味保存や停止性の証明ではない | [検証](verification.md)・[proof](../proof/README.md) |
| 最適化と助言 | typed IRの簡約、到達性による除去、wasm-opt向け出力、明示指定の性能警告 | 外部最適化は任意。警告は助言で、動作や速度の保証ではない | [wasm-opt](wasm-opt.md) |

## 非同期・並行・並列

| 機能 | 現在できること | 規約・制限 | 詳細 |
| --- | --- | --- | --- |
| Future・Stream | 所有する端点、ready値、外部Promise/AsyncIterableとの接続、writer・batch転送 | payloadは共有値。Streamの終端でもcloseが必要 | [ランタイム](runtime.md)・[batch](stream-batches.md) |
| 子タスク | スコープ付きspawn/join、取消し、期限、backpressure | 同じインスタンス内の協調並行。OSスレッドを生成する命令ではない | [子タスク](scoped-tasks.md) |
| Wasm threads | shared memory、atomic load/store/RMW/cmpxchg/wait/notify/fence | Worker生成・配送はホスト側。GC参照をエージェント間で共有しない | [threads](threads.md) |
| race-freeの部分集合 | 固定atomicセル、公開後不変の領域とchecked loadの契約 | 任意のraw pointerアクセスは対象外。ホストも同じ契約に従う | [atomic cells](atomic-cells.md)・[readonly](readonly-regions.md) |
| Workerホスト | Node worker_threadsとブラウザWorkerのpool・queue・cancel・join | driverは明示したESM。WLTのclosureをそのまま転送しない | [Worker host](worker-host.md) |

## 出力と外部連携

| 入口 | 用途 | 現在の境界 | 詳細 |
| --- | --- | --- | --- |
| Core Wasm | VMへ組み込む、数値・GC・線形メモリを扱う | ホストimportは別途供給。GC参照を自動でJS objectに変換しない | [CLI](cli.md) |
| Core＋JSPI | JS値のcodec、Promise、タスク取消し付きの軽量ランタイム | 公開呼び出しの直列化やABI値の制約を守る | [ランタイム](runtime.md) |
| Component・WIT | WITの型付きbinding生成、対応するworldへの出力、native async/resource | SDKとnative worldで対応capabilityが異なる。WASI一式の実装ではない | [binding](wit-bindings.md)・[native](native-worlds.md) |
| WIT付き`.wasm`の`%use` | Componentの同期・共有値のexportを型付きで静的に取り込む | 任意のCore Wasmの推測読み込みではない。閉じたComponentなどの制限がある | [Wasm依存](wasm-use.md) |
| 明示したI/O | CLIのstdio、許可したファイル名、ホストcapability | OS資源やネットワークへの暗黙の権限は与えない | [CLI](cli.md)・[WASI値adapter](wasi-files.md) |
| ViteのESM import | `.wlt`のnamed export、`?init`、dev更新、本番asset、SSR、型宣言生成 | Vite 8.3以降と対応CLI。GC ABIと共有メモリhost契約の制限を引き継ぐ | [Vite](vite.md) |
| TS7 Content Mapper | 宣言ファイルなしの型検査、exportへのナビゲーション、watch | 対応するnative TS7が必要。依存WLTの未保存bufferは一括で渡さない | [Content Mapper](content-mapper.md) |

result/option/future/streamの形はWASI 0.3・WITとの変換を意識している。
これらの型があることと、すべてのWASI APIをそのまま実行できることは別。

## 開発ツールとサンプル

| 入口 | できること | 詳細 |
| --- | --- | --- |
| `wlt check`・`types`・`type-at`・`analyze` | 構造化診断、型情報、解析結果 | [CLI](cli.md)・[ツールAPI](tooling.md) |
| `wlt test`・`wlt bench` | 言語内のテスト、Wasm内で反復するベンチ | [テスト](testing.md)・[bench](benchmarking.md) |
| `wlt lsp` | 診断、hover、定義移動、補完、マクロ展開、未完bufferの宣言回復 | [LSP](lsp.md) |
| Playground | Wasm版コンパイラによる編集・検査・実行と機能ツアー | [Playground](playground.md) |
| 処理系の計測・評価 | Criterion、guest profiling、読解・診断・記述課題の固定評価 | [性能](frontend-performance.md)・[evals](../evals/README.md) |

小さい例は[サンプルカタログ](../examples/catalog.json)、複数ファイルの例は
[CLIプロジェクト](../examples/cli/README.md)・[ログ集計](../examples/apps/log-stats/README.md)・
[在庫台帳](../examples/apps/stock-ledger/README.md)・[workflow](../examples/apps/workflow/README.md)を参照。
残る課題は[TODO](../TODO.md)。過去の評価・計測ファイルは当時の入力と結果を保存しているので、現在のAPI一覧としては使わない。
