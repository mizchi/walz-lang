# ファイルモジュール・%use / %export と DCE

`.wlt`の定義に加え、WIT付きの閉じたComponent `.wasm`からも同期・共有値のexportを取り込める。`(%use "./math.wasm" (func $math.add $add))`の型・effect・出力先の契約は[Wasmの%use](wasm-use.md)にまとめる。

`%include` は宣言を挿入するテキスト展開です。スコープや読み込み済みキャッシュを作りません。
関数・型・tag・メモリを選んで使うときは、明示的な相対パスの `%use` を使います。

`math.wlt`:

```wat
(module
  (%export (func $answer) (func $unused))
  (func $helper (result i32) (%effect pure) (i32.const 42))
  (func $answer (result i32) (%effect pure) (call $helper))
  (func $unused (result i32) (%effect pure) (i32.const 99)))
```

`main.wlt`:

```wat
(module
  (%use "./math.wlt" (func $answer $chosen))
  (export "main" (func $chosen)))
```

静的 export の `$answer` を `$chosen` に束縛し、それを Wasm の `main` として公開します。
生成物には answer とその依存の helper が入り、unused は入りません。呼出し元で `$helper`・`$unused` は使えません。
[実行できるサンプル](../examples/modules.wlt) は `just run examples/modules.wlt` で42を返します。

## 境界の規約

| 宣言 | 意味 |
| --- | --- |
| `(%export (func $f) (type $T) (tag $Error))` | 静的に公開する名前。`%public` も互換で受理 |
| `(%export (opaque $State))` | 型を公開し、その表現を宣言スコープに閉じる |
| `(%use "./file.wlt" (func $f $local) (type $T) (tag $Error))` | ファイル API の選択と別名。別名省略時は同じ識別子 |
| `(%use "wlt:core" (func $string.u64 $unsigned))` | 組み込みコアから公開された識別子を選択 |
| `(export "name" (func $local))` | ルートから Wasm / Component API に公開。静的 import の関数も指定可能 |
| 関数内の `(export "name")` | 従来どおりの Wasm ABI export |
| `(import "host" "member" (func ...))` | 従来どおりの Wasm / WIT ホスト import |

ファイルは独立した静的スコープです。読み込んだファイルの宣言 は呼出し元に流入しません。
依存側が import した名前も、export しなければ外へ出ません。
同じ canonical path は1つの定義集合を共有します。別名・`./`・複数の経路でも型を作り直しません。
opaque 型の所有スコープも維持します。絶対パスと循環 import は拒否し、include と合わせて深さ64・展開後16MiBの上限を適用します。
ファイル名の解決・診断の元ファイルへの対応は `source::load` が担当し、文字列 API の check/compile はファイル I/O を行いません。

再公開は `(%use "./source.wlt" (func $original $renamed))` と `(%export (func $renamed))` を組み合わせます。
`%namespace` は廃止しました。1ファイルが1モジュールです。`%use` の対象は `wlt:core` などの登録済みパッケージか、`./`・`../` で始まるファイルパスです。
ドットは識別子の一部なので、`$stock.get` も明示的に束縛しなければ使えません。ファイル内に宣言した namespace を探索する規則はありません。
変数の再代入、トップレベル実行、start、メモリ初期化などをこの静的ファイルモジュールに追加していません。
wildcard・default import/export・dynamic import・循環モジュールは未対応です。
取り込むライブラリ内の `%test` も拒否します。テストは別の入口ファイルから API を import して書き、生成物の根に意図せず追加されることを防ぎます。

## 生成時の除去

先に読み込んだ全ソースを名前解決し、型・effect・所有権・opaque・必要な law を検査します。
使わない関数にある不正な型、effect、未消費の所有値も拒否します。
その後、ABI export・テスト・law を根として、直接呼出し、クロージャ、型のフィールド、値型の payload、例外 tag と生成 helper の依存をたどります。
宣言を落として ID を振り直し、同じ投影を Core Wasm、Core＋JSPI、Component、プロファイラの ABI に使います。
到達しないホスト関数も manifest・codec・生成 import から除き、不要な capability を要求しません。
Component の trusted binding 検査は削除前にも適用するため、未対応のホスト binding を含むライブラリの受理範囲は広げません。
`check` / `ir` / `analyze` は削除前の全ソースを保持します。import したという理由だけで生成物の根にはしません。
この到達性解析と ID の再割当てはコンパイラの TCB です。Lean の契約カーネルによる意味保存の証明は追加していません。

標準 WAT 単体は従来どおり書き換えません。start・メモリ・table・global や外部の import 解決を含む任意の WAT に、この除去を適用することはありません。
`%include` の結果が標準 WAT だけになる場合も、その経路を維持します。named file import は型付きの静的リンク経路です。

Wasm の import はインスタンス化時に必要な外部定義であり、start 等も実行されます。[Wasm Core 仕様](https://www.w3.org/TR/wasm-core/#modules)
ESM の import は別モジュールの binding への参照で、値をコピーする宣言ではありません。[ECMAScript FAQ](https://github.com/tc39/ecma262/blob/main/FAQ.md#why-are-imported-module-bindings-aliased-instead-of-copied)
WLT はこの名前の明示的な束縛を採用し、初期化を持たない関数・型・tag の部分集合を単一 Wasm へ静的リンクします。ESM 全体の実行モデルを実装するものではありません。

`just test-modules` でファイル共有・可視性・再公開・未使用 import の除去と両 JS backend の実行を確認できます。
ブラウザでホスト capability を与えず動作する検査も `just test-e2e` に含まれます。

## 標準ライブラリの生成サイズ

以下は旧入力に対する named import 導入時の記録です。現在のソース・構文に対する測定値ではありません。
JSON builder 共通化後の保存済み成果物と、今回の成果物を同じ入力・期待値で比較しています。
[入力・ソース・生成物のハッシュと比較結果](stdlib/20261105-imports-dce.json) を保存しました。
すべての呼出しで JSON 出力・入力終了1回・ABI の未解放数0を照合しています。

| Core Wasm（名前セクション込み） | 変更前 bytes | named import ＋ DCE 後 bytes |
| --- | ---: | ---: |
| ログ集計 | 13,942 | 11,711 |
| 在庫台帳 | 15,192 | 13,065 |
| workflow | 22,966 | 20,939 |

速度の優劣は今回の変更の判定基準にしていません。関数・型・ホスト import の到達性を対象とし、式の簡約や共有する生成 helper の完全な最小化は別の最適化として残します。

回帰検査は `just check`、Rust 212件、言語内テスト41件、Node 45件、Chromium 26件、評価ハーネス11件が成功しました。
未使用 import を除いた Component で生成されなくなる Future / Stream の管理表も、検査用メトリクスでは不在を0として扱います。

`%export (memory $mem)` と `%use "./file.wlt" (memory $mem $local)` でメモリも選択できる。ライブラリ内の memory 命令は参照先の名前を明示する。[共有メモリ](threads.md) を参照。

固定atomicセルは`(cell $name $alias)`、readonly領域は`(region $name $alias)`で選択できる。領域を取り込むと対応するmemoryもリンクする。これらは整数ポインタを公開する操作ではない。[atomicセル](atomic-cells.md)・[readonly領域](readonly-regions.md)を参照。
