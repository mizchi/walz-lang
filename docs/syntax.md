# `%` で WLT 拡張を区別する

WLT が追加する構文の入口には `%` を付ける。接頭辞のない `import`、`export`、`loop`、`try`、`result` は WAT の意味を保つ。識別子の `$name`、コメント、文字列の内容は変えない。

| 用途 | 構文 |
| --- | --- |
| 式・制御構造 | `%let`、`%for`、`%loop`、`%recur`、`%match`、`%try` |
| 値の操作 | `%list.new`、`%text.lit`、`%result.ok`、`%future.await` など |
| 値型 | `%text`、`%bytes`、`%unit`、`(%list T)`、`(%option T)`、`(%result T E)` など |
| 宣言・注釈 | `%closure`、`%capture`、`%effects`、`%type.alias`、`%type.params`、`%where`、`%test`、`%law` |
| 静的モジュール | `%include`、`%import`、`%export`、`%namespace`、`%public`、`%use` |
| 型引数の適用 | `(%inst $name T...)`。関数・型・alias・クロージャで共通 |

拡張の内部にある `case`、`bind`、`while`、`step`、`catch`、制約の `shared` / `nonunit` / `callable` は、その入口に属するデータ構文なので接頭辞を付けない。`if`、`then`、`else`、`block`、`param`、結果宣言の `result` は WAT の構文を使う。

```wat
(module
  (func $id (%type.params $T) (%where (shared $T))
    (param $x $T) (result $T) (%effects pure)
    (local.get $x))
  (func $main (export "main") (result i64) (%effects pure)
    (%let $answer (call (%inst $id i64) (i64.const 42))
      (local.get $answer))))
```

`(result i32 i64)` は標準の複数戻り値宣言。`(result (%result i32 i64))` は WLT の Result 値を1つ返す宣言。

```wat
(%import "math.wlt" (func $answer))  ;; ソースの静的リンク
(%export (func $answer))            ;; ソース API に公開
(import "host" "answer" (func $host-answer (result i32)))
(export "answer" (func $answer))    ;; Wasm API に公開
```

[Type Imports 提案](https://github.com/WebAssembly/proposal-type-imports/blob/main/proposals/type-imports/Overview.md) の `(import "module" "name" (type ...))` も WAT 側の構文として扱い、ファイル読み込みに解釈しない。提案そのものへの対応範囲は固定した `wat` / `wasmparser` に依存する。この変更は type imports の実装を追加するものではない。

## 標準 WAT との混在と解析

標準 WAT 単体は従来どおり、そのまま検証・コンパイルする。型付きコアでは型・effect・所有権・law の既存の検査を行う。

型付きコアの外にある WAT には、構文だけで展開できる `%let`、状態付き `%loop` / `%recur`、`%for` と静的な名前・型の展開を併用できる。展開したモジュール全体を Wasm validator で検査する。`%let` の型が推論できない初期値には明示的な型注釈を書く。

この経路は `analysis.opaque=true`、保守的な全 effect、`ir=None` を返す。`%effects`、`%where`、所有値、クロージャ、値型、テスト、law などの検査を必要とする拡張をこの経路で処理しない。Component / Core＋JSPI / verified の生成は型付きコアを必要とする。

`%let` とループ状態は不変。標準の `local.set` / `local.tee` から変更できない。数値ローカルインデックスは元の WAT の引数・ローカルだけを指し、生成した一時変数には届かない。`%let` は制御ラベルを追加しない。`%recur` は最内側の WLT ループへの末尾呼び出しで、引数をすべて評価してから同時に状態を更新する。

## 旧構文からの移行

```sh
just migrate examples/old.wlt        # 変更を標準出力で確認
cargo run -- migrate examples/old.wlt --write
cargo run -- --legacy-syntax check examples/old.wlt
```

通常の入力では旧構文を `E_WLT_PREFIX` として拒否する。未知の `%` 構文は `E_WLT_EXTENSION`。自動的な旧構文への切り替えはしない。

`migrate` は1ファイルへの挿入だけを行い、コメント・文字列・識別子を保持する。依存ファイルはそれぞれ移行する。検証可能な標準 WAT は変更しない。WAT と字面の等しい旧 `try` / `catch_all` は標準構文として保持するため、WLT の例外マクロとして使う場合は `%try` と明示する。

`--legacy-syntax` はファイルグラフを読み込んでからメモリ上で移行し、元ファイルは変更しない。診断の位置も元ファイルに対応させる。評価の凍結済み入力はこのオプションで実行し、過去の入力・結果・スナップショットは保存する。新規の可読性評価は suite v5 として区別する。

`just test-syntax` で標準 WAT の保持、混在時の実行、移行、診断位置、検査の境界を確認できる。
