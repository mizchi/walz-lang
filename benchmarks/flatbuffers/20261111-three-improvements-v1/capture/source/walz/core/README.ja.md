# 組み込みコアライブラリ `walz:core`

[English](README.md) | 日本語

基本型は既存の基底型への透明な alias、標準関数は通常の Walz 関数です。ライブラリごとの追加ランタイムは持ちません。
必要な関数・型を列挙して、`walz:core` から import します。コンパイラが同梱したソースを静的にリンクするため、呼出し元の場所やインストール先のパスに依存しません。
Option→Resultとエラー変換は `$option.ok-or` / `$result.map-err` を使います。[評価順と制約](../../docs/option-result.md)。
完全な型・型制約・effect は [生成した標準 API 一覧](../../docs/stdlib-api.md) を参照してください。`just stdlib-docs` で公開宣言から更新し、`just check-stdlib-docs` でソースとの一致を確認します。
標準文字列型は `$string` です。`bytes-view.walz` / `string-view.walz` は [ゼロコピービュー](../../docs/views.md) を提供し、部分範囲の共有と明示的なコピーを使い分けます。
`dict.walz` はtextキーの汎用辞書、`list-builder.walz` は不変listの構築を提供します。型制約・変更とスナップショットの契約は [コレクション](../../docs/collections.md) を参照してください。

```wat
(module
  (@use "walz:core" (type $string) (func $string.u64 $unsigned))
  (func $answer (export "answer") (result $string) (@effect reads writes allocates)
    (call $unsigned (i64.const 42))))
```

各ファイルの `@export` が API を定義します。型名は `$string-view` のような小文字のkebab-case、関連関数は `$string-view.new` のような `型名.操作名` に揃えます。公開名を選び、任意のローカル名へ束縛できます。[WITとの命名対応](../../docs/naming.md)を参照してください。
`json.walz` → `builder.walz` → `text.walz` → `bytes.walz` の依存も named import です。同じファイルはコンパイル単位内で一度だけ読み込まれ、型と定義を共有します。
選ばなかった名前は呼出し元に束縛されません。生成する Wasm は実際の ABI export・テスト・law から到達する宣言だけを保持します。
詳細は [静的モジュールと DCE](../../docs/static-modules.md) を参照してください。`include` は従来のテキスト挿入として残します。

| import 元 | 関数 | 結果・契約 |
| --- | --- | --- |
| bytes.walz | `$bytes.equal(a,b)` | bytes の内容一致。i32 の0/1 |
| bytes.walz | `$bytes.find-byte(b,start,byte)` | 最初の位置、なければ b の長さ。範囲外の start でも長さ。byte は0–255、他の値は一致しない |
| bytes.walz | `$bytes.find(b,needle,start)` | option<i32>。start 以降の最初の一致。範囲外は none。空 needle は有効な start に一致 |
| bytes.walz | `$bytes.split-once(b,needle)` | option<pair<bytes,bytes>>。最初の区切りを取り除く。見つからなければ none |
| bytes.walz | `$bytes.parse-u64(b,start,length,limit)` | 範囲内の ASCII 十進数全体を読む。上限は符号なし i64。空・不正範囲・不正文字・桁あふれは none |
| bytes.walz | `$bytes.concat(a,b)` | 連結した bytes |
| text.walz | `$string.find(s,needle,start)` | option<i32>。UTF-8 の byte 位置。start が文字の途中または範囲外なら none |
| text.walz | `$string.split-once(s,needle)` | option<pair<text,text>>。空の前後フィールドを保持し、残りの区切りは分割しない |
| text.walz | `$string.parse-u64(s)` / `$string.parse-s64(s)` | option<i64>。u64 全域／s64 全域。signed だけ先頭の `-` を許す |
| text.walz | `$string.concat(a,b)` | text の連結 |
| text.walz | `$string.u64(n)` / `$string.s64(n)` | 符号なし／符号付き十進数の text |
| json-value.walz | `$json.from-string(s)` / `$json.stringify(value)` | JSONのstring値を構築／JSONテキストを生成 |
| builder.walz | `$string-builder.new()` | 不透明な可変 GC 型 `(ref $string-builder)` |
| builder.walz | `$string-builder.text(b,s)` | text をそのまま追加。戻り値は unit。JSONは `$json.stringify` と組み合わせる |
| builder.walz | `$string-builder.u64(b,n)` / `$string-builder.s64(b,n)` | 十進数を追加。戻り値は unit |
| builder.walz | `$string-builder.finish(b)` | 結合した不変 text を返して空に戻す。繰り返すと空 text |
| json.walz | `$json.object-builder.new()` | 不透明な可変 GC 型 `(ref $json-object)`。空 object |
| json.walz | `$json.object-builder.string(o,key,value)` / `$json.object-builder.u64(o,key,n)` / `$json.object-builder.s64(o,key,n)` | 型に合うフィールドを追加。キーもエスケープする。戻り値は unit |
| json.walz | `$json.object-builder.boolean(o,key,n)` | 0 は false、それ以外は true。戻り値は unit |
| json.walz | `$json.object-builder.object(o,key,child)` | child を finish して、その JSON object を値として追加 |
| json.walz | `$json.object-builder.finish(o)` | JSON object の text を返し、空 object へ戻す。繰り返すと `{}` |

検索・分割・整数解析は semantic effect が `pure` です。値を格納する物理的な GC 割当てはあり得ます。
位置・長さは i32 の符号なしビットパターンとして扱います。`-1` は最大値となり、解析では不正な範囲です。
文字列の正規化や Unicode の文字数・書記素の操作は行いません。空の区切りでの split は `(空, 元の値)` を返します。
bytes の split は任意の byte 列を扱い、text の split は有効な UTF-8 の境界を保ちます。
find-byte と整数解析は走査長に比例します。部分列の find は単純な比較で、最悪時は入力長×needle 長、split はさらに前後の値をコピーします。

整数解析は先頭ゼロを許し、空白・`+`・区切り・非 ASCII 数字を許しません。`-0` は signed で0です。
乗算前に上限との比較を行い、wrap した値を成功扱いしません。
Wasm の i64 に符号の区別はないため、u64 の成功値も i64 のビットパターンです。WIT の s64 / JS BigInt 境界では最上位 bit が符号として見えます。

builder と JSON object は共有可能な可変 GC オブジェクトです。別名も同じ状態を観測します。構築・変更・finish は reads/writes/allocates を持ちます。
finish 済みの text は以後の変更と独立し、builder は以前の断片への参照を保持しません。
空の追加は無視し、断片数に比例するリストと、出力長に比例する結合処理を使います。繰り返すたびに既存の全文をコピーする方式にはしません。
合計長の u32 桁あふれや GC 配列の確保失敗は Wasm の trap です。

JSON のフィールドは追加順に並びます。同名のキーはそのまま出力します。外部の JSON text を未検査で埋め込む API は用意していません。
`$json.object-builder.object` は child を空へ戻します。parent と child が同じ別名でも、変更前の内容を有限の値として取り込んでからフィールドを追加するので循環を作りません。
object・string・整数・boolean に加え、検査付き number、null、JSON 値を追加できます。JSON の数値は text として保持します。

`just test-stdlib` で Wasmtime と Core / Component の実行を確認します。ブラウザは `just test-e2e` に含まれます。
`just stdlib-capture DIRECTORY` で3アプリの生成物と入力・期待値・ソースのハッシュを固定し、変更後に別名で capture します。
`just stdlib-compare BEFORE AFTER` は固定した Core＋JSPI 成果物を交互に測定し、各呼出しの出力・入力終了・ABI の未解放数0を照合します。

`json-value.walz` は不変の JSON 値、parser、配列・object の構築と参照、stringify を提供します。
`json.walz` の object builder は `null`、検査付き `number`、任意の JSON `value` も書けます。
数値の精度、入力上限、API は [JSON 値と parser](../../docs/json-values.md) を参照してください。
