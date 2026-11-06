# 実装

```text
ファイル import / include → wlt:std の静的展開 → S 式・固定構文の正規化
  → 名前解決・単相化・型検査 → 型付き IR
                              ├─ 全ソースの解析・契約検査
                              └─ 到達性による宣言の除去 → WAT → Wasm
```

- `src/ir.rs`: 型、ローカル・関数・ループの ID、閉じた式の列挙型。解析と出力の共通コントラクト。
- `src/sexpr.rs`: 位置情報付きの S 式リーダー。`;;` と入れ子の `(; ;)` コメントに対応。
- `src/source.rs`: 相対 import / include、ファイルの共有と循環検出、診断の元ファイルへの対応。
- [src/standard.rs](../src/standard.rs): 同梱する `wlt:std` の型 alias と標準関数を選択し、通常の静的モジュールへ展開。ファイル探索や実行時の初期化は行わない。
- [src/syntax.rs](../src/syntax.rs): `%` で始まる入口の正規化と、複数束縛の `%let` から既存のネストした束縛への展開。
- `src/modules.rs`: 種類付きの名前表で定義と import 別名を分け、収集・import 解決・宣言の展開の順に処理。
- `src/generics.rs`: 明示した型引数ごとの具体化、再利用、再帰・展開上限と診断。実行用 IR に汎用宣言を残さない。
- `src/bounds.rs`: where の shared/nonunit/callable と effect 上限を、具体的な型・署名・推論 effect で検査。
- `src/type_syntax.rs`: 値型の位置と nominal な関数・heap 型の参照を区別する共通 visitor。
- `src/link.rs`: 検査済みの全 IR から到達する宣言を選び、生成用の ID を振り直す。
- `src/buffers.rs`: bytes/text の private GC storage、UTF-8 検証と値操作。
- `src/frontend.rs`: 型・名前・捕捉・可変性・match の網羅性・tail position の検査。
- `src/typing.rs`: 宣言・推論した型の一覧と、ソース位置からの型の問い合わせ。
- `src/inspect.rs`: 元ファイルの行・列から、検査済みIRと元の型注釈・具体化位置を照合する読み取り専用API。
- `src/cli_inspection.rs`: JSON診断とtype-atのCLI入出力。検査ロジックと表示用メタデータを分離する。
- `src/native.rs`: 型付きコア外の標準 WAT の関数シグネチャ。
- [src/native_macros.rs](../src/native_macros.rs): コア外の WAT と構文だけのマクロの混在。全体を Wasm validator で検証し、解析結果を opaque とする。
- `src/runner.rs`: テスト探索、分離した実行、タイムアウト、構造化した結果。
- `src/analysis.rs`: 副作用と呼び出しグラフの解析。文字列の WAT を再解析せず、型付き IR を走査します。
- `src/regions.rs`: 対象別の読み書き、参照の流れ、割当て箇所と escape の解析。
- `src/flow.rs`: 正常な制御フロー上の non-null、i32 区間と配列境界の解析。
- `src/effects.rs`: ソースで観測できる作用、関数外へ出る例外、宣言した effect 上限の検査。
- `src/ownership.rs`: 所有性の伝播、明示 move、分岐・ループ・終了時の所有集合の契約。
- `src/proof.rs`: 型付き IR から BitVec による law の証明義務を生成。
- `src/verified.rs`: Lean の検査を通した同じ IR を Wasm へ出力する経路。
- `proof/Ownership.lean`: 所有権の小さな検査器、宣言的な規則、検査の健全性と二重消費拒否の証明。
- `proof/Delivery.lean`: 配送済み Future/Stream の操作、順序・終端・close のモデルと証明。
- `src/wit.rs`: 公開関数の型から WIT 契約を生成。
- `src/bindings.rs`: event-loop import の固定した型と作用。
- `src/component.rs`: 検査済みの型と core Wasm から Component と manifest を組み立てる。
- `src/component/abi.rs`: portable な型、Canonical ABI のレイアウト・flattening・シグネチャ。
- `src/component/values.rs`: GC 値と ABI 値の変換、bytes/text と複合値のコピー・バッファ解放。
- `src/component/endpoints.rs`: readable handle の転送・read・close。
- `src/component/functions.rs`: core import/export の境界とタスクの cleanup。
- `src/component/core_js.rs`: 検査済みの IR から JS 専用の core Wasm と bindings を生成。
- `src/component/core_js/values.rs`: 必要な具体型の JS codec を生成。
- `src/component-memory.wat`: 変換用バッファの確保・解放。
- `runtime/text-io.mjs` / `node-io.mjs` / `text-cli.mjs`: 値だけを渡す行 I/O と、Node の handle・CLI。
- `runtime/core.mjs` / `build-core.mjs`: JS 専用の Core/JSPI ホストとビルド。状態はインスタンスごとに分離。
- `runtime/`: Component の JSPI 向けの生成、共通の Promise/AsyncIterable ホスト、タスクの要求・終了確認・解放、Node とブラウザの実行テスト。
- `proof/Tasks.lean`: 取消し要求と終了確認の状態モデル、read 終了前のバッファ解放拒否、解放前の終了確認拒否の証明。
- `src/emit.rs`: GC 型、let、for、クロージャ、enum/match、try/catch を通常の WAT に変換。
- `src/lib.rs`: `check(&str)` は型付きコアの IR、`compile(&str)` は標準 WAT と拡張の両方に対応します。Wasm は wasmparser でも検証します。

列挙型はタグとケース別の GC subtype へ変換します。bytes/text は private な packed GC 配列を使い、text は UTF-8 を検証します。
利用者定義の関数・型・alias・クロージャは、`(%type.params $T ...)` と `(%inst $name T...)` により単相化します。
具体的なクロージャ型を引数にする [map/fold の実例](../examples/generic-lists.wlt) と
[具体化時の検査規則](generics.md) を参照してください。
異なるクロージャ型を共通のインターフェースで呼ぶ糖衣、finally、共有 GC ヒープのスレッド間利用の糖衣は、この核の外に置いています。

次の型・effect 設計は Verse の effect family と WASI 0.3.0 の WIT に合わせます。
Option/Result は値、Future/Stream は所有する配送ハンドル、suspends は制御上の作用として分けます。
設計規約・実装範囲は [SPEC.md](../SPEC.md)、形状契約は [async-contract.wit](../interfaces/async-contract.wit) を参照してください。
値型・effect 契約・所有権・law・配送済み/外部の Future/Stream、WIT 生成、Component ABI adapter、JSPI でのタイマー実行と task cancellation は実装済みです。writer と言語側の producer task は今後の範囲です。
`just wit-check` と WIT のテストには `wasm-tools` が必要で、検証済みバージョンは1.245.1です。
