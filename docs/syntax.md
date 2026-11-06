# `%` で WLT 拡張を区別する

WLT が追加する構文の入口には `%` を付ける。接頭辞のない `import`、`export`、`loop`、`try`、`result` は WAT の意味を保つ。識別子の `$name`、コメント、文字列の内容は変えない。
固定の記号演算子`+`・`-`・`*`・`/`・`=`・`!=`・`<`・`<=`・`>`・`>=`・`&&`・`||`・`&`・`|`・`^`・`<<`・`>>`・`>>>`と、式位置のローカル・数値の省略形は接頭辞を持たない糖衣とする。

| 用途 | 構文 |
| --- | --- |
| 式・制御構造 | `%let`、`%for`、`%loop`、`%recur`、`%match`、`%try`、`%result.do` |
| 値の操作 | `%list.new`、`%string.lit`、`%result.ok`、`%future.await` など |
| struct構築 | `(%struct.new $T ($field 式) ...)`。`{}`のfield群と`(%$T::{...})`の短縮形も使える |
| 値型 | `$string`、`$bytes`、`$unit`、`(%inst $list T)`、`(%inst $option T)`、`(%inst $result T E)` など |
| 宣言・注釈 | `%closure`、`%capture`、`%effect`、`%newtype`、`%type.alias`、`%type.params`、`%where`、`%test`、`%law` |
| 静的モジュール | `%include`、`%use`、`%export`、`%public` |
| 型引数の適用 | `(%inst $name T...)`。関数・型・alias・クロージャで共通 |

拡張の内部にある `case`、`bind`、`while`、`step`、`catch`と`%where`の制約名は、その入口に属するデータ構文なので接頭辞を付けない。`if`、`then`、`else`、`block`、`param`、結果宣言の `result` は WAT の構文を使う。

型付きコアの式位置では、`$foo`を`local.get`の省略形、期待するスカラー型が分かる数値を`.const`の省略形として使える。letの型注釈は`(%let $x (type i32) 1 body)`と書ける。この`(type T)`は`%let`に属する注釈であり、独立した命令ではない。[規約と例](core-expressions.md#ローカル参照と数値の省略形)。

基本型は `(%use "wlt:core" (type $result) (type $option) (type $future) ...)` で明示的に取り込む。Result と Future は `(%inst $result T E)` / `(%inst $future T)`、型引数のない text は `$string` と参照する。複数束縛は `(%let (($a initial-a) ($b initial-b)) body)` で左から右に初期化する。[標準モジュールと束縛](standard.md) に詳しい規約をまとめた。

```wat
(module
  (func $id (%type.params $T) (%where (shared $T))
    (param $x $T) (result $T) (%effect pure)
    (local.get $x))
  (func $main (export "main") (result i64) (%effect pure)
    (%let $answer (call (%inst $id i64) (i64.const 42))
      (local.get $answer))))
```

`(result i32 i64)` は標準の複数戻り値宣言。`(result (%inst $result i32 i64))` は WLT の Result 値を1つ返す宣言。
型付きコアの戻り値は0個（unit）または1値に限る。pair は1つの値として返し、pair の分解 let で取り出す。標準 WAT の複数戻り値は標準 WAT 経路で検証・コンパイルし、型付きコアの複数束縛へ自動変換しない。

```wat
(%use "./math.wlt" (func $answer))  ;; ソースの静的リンク
(%export (func $answer))            ;; ソース API に公開
(import "host" "answer" (func $host-answer (result i32)))
(export "answer" (func $answer))    ;; Wasm API に公開
```

[Type Imports 提案](https://github.com/WebAssembly/proposal-type-imports/blob/main/proposals/type-imports/Overview.md) の `(import "module" "name" (type ...))` も WAT 側の構文として扱い、ファイル読み込みに解釈しない。提案そのものへの対応範囲は固定した `wat` / `wasmparser` に依存する。この変更は type imports の実装を追加するものではない。

## 標準 WAT との混在と解析

標準 WAT 単体は従来どおり、そのまま検証・コンパイルする。型付きコアでは型・effect・所有権・law の既存の検査を行う。

型付きコアの外にある WAT には、構文だけで展開できる `%let`、状態付き `%loop` / `%recur`、`%for` と静的な名前・型の展開を併用できる。展開したモジュール全体を Wasm validator で検査する。`%let` の型が推論できない初期値には明示的な型注釈を書く。

この経路は `analysis.opaque=true`、保守的な全 effect、`ir=None` を返す。`%effect`、`%where`、所有値、クロージャ、値型、テスト、law などの検査を必要とする拡張をこの経路で処理しない。Component / Core＋JSPI / verified の生成は型付きコアを必要とする。

`%let` とループ状態は不変。標準の `local.set` / `local.tee` から変更できない。数値ローカルインデックスは元の WAT の引数・ローカルだけを指し、生成した一時変数には届かない。`%let` は制御ラベルを追加しない。`%recur` は最内側の WLT ループへの末尾呼び出しで、引数をすべて評価してから同時に状態を更新する。

## 入力の規約

WLT 拡張には `%` を必須とし、接頭辞の欠落を `E_WLT_PREFIX` として診断する。未知の入口は `E_WLT_EXTENSION`。移行コマンドや旧構文との互換性を言語仕様には設けない。

凍結済みの評価入力を再実行するための変換だけを、評価ハーネス用の内部処理に残す。過去の入力・結果・スナップショットは保存する。標準型の import と複数束縛を質問した suite v7 に続き、標準APIの選択とi64カウンタへの変更を含む評価は suite v8 として区別する。

`just test-syntax` で標準 WAT の保持、混在時の実行、診断位置、検査の境界を確認できる。

数値命令・return・ラベル付き分岐、Result の平坦な連鎖と scalar newtype は [型付きコアの式](core-expressions.md) を参照。
