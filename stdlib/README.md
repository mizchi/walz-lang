# 標準ライブラリ

すべて通常の WLT 関数です。コンパイラの命令・型・ランタイムは追加しません。
必要な関数・型を列挙して、ファイルから import します。パスは呼出し元からの相対位置です。

```wat
(module
  (import "stdlib/text.wlt" (func $string.u64 $unsigned))
  (func $answer (export "answer") (result text) (effects reads writes allocates)
    (call $unsigned (i64.const 42))))
```

各ファイルの namespace 内にある `export` が API を定義します。`$string.u64` のような公開パスを選び、任意のローカル名へ束縛できます。
`json.wlt` → `builder.wlt` → `text.wlt` → `bytes.wlt` の依存も named import です。同じファイルはコンパイル単位内で一度だけ読み込まれ、型と定義を共有します。
選ばなかった名前は呼出し元に束縛されません。生成する Wasm は実際の ABI export・テスト・law から到達する宣言だけを保持します。
詳細は [静的モジュールと DCE](../docs/static-modules.md) を参照してください。`include` は従来のテキスト挿入として残します。

| import 元 | 関数 | 結果・契約 |
| --- | --- | --- |
| bytes.wlt | `$buffer.equal(a,b)` | bytes の内容一致。i32 の0/1 |
| bytes.wlt | `$buffer.find-byte(b,start,byte)` | 最初の位置、なければ b の長さ。範囲外の start でも長さ。byte は0–255、他の値は一致しない |
| bytes.wlt | `$buffer.find(b,needle,start)` | option<i32>。start 以降の最初の一致。範囲外は none。空 needle は有効な start に一致 |
| bytes.wlt | `$buffer.split-once(b,needle)` | option<pair<bytes,bytes>>。最初の区切りを取り除く。見つからなければ none |
| bytes.wlt | `$buffer.parse-u64(b,start,length,limit)` | 範囲内の ASCII 十進数全体を読む。上限は符号なし i64。空・不正範囲・不正文字・桁あふれは none |
| bytes.wlt | `$buffer.concat(a,b)` | 連結した bytes |
| text.wlt | `$string.find(s,needle,start)` | option<i32>。UTF-8 の byte 位置。start が文字の途中または範囲外なら none |
| text.wlt | `$string.split-once(s,needle)` | option<pair<text,text>>。空の前後フィールドを保持し、残りの区切りは分割しない |
| text.wlt | `$string.parse-u64(s)` / `$string.parse-s64(s)` | option<i64>。u64 全域／s64 全域。signed だけ先頭の `-` を許す |
| text.wlt | `$string.concat(a,b)` | text の連結 |
| text.wlt | `$string.u64(n)` / `$string.s64(n)` | 符号なし／符号付き十進数の text |
| text.wlt | `$string.json(s)` | 引用符込みの JSON string |
| builder.wlt | `$builder.new()` | 不透明な可変 GC 型 `(ref $builder.Builder)` |
| builder.wlt | `$builder.text(b,s)` / `$builder.json(b,s)` | text をそのまま／JSON string にして追加。戻り値は unit |
| builder.wlt | `$builder.u64(b,n)` / `$builder.s64(b,n)` | 十進数を追加。戻り値は unit |
| builder.wlt | `$builder.finish(b)` | 結合した不変 text を返して空に戻す。繰り返すと空 text |
| json.wlt | `$json.new()` | 不透明な可変 GC 型 `(ref $json.Object)`。空 object |
| json.wlt | `$json.string(o,key,value)` / `$json.u64(o,key,n)` / `$json.s64(o,key,n)` | 型に合うフィールドを追加。キーもエスケープする。戻り値は unit |
| json.wlt | `$json.boolean(o,key,n)` | 0 は false、それ以外は true。戻り値は unit |
| json.wlt | `$json.object(o,key,child)` | child を finish して、その JSON object を値として追加 |
| json.wlt | `$json.finish(o)` | JSON object の text を返し、空 object へ戻す。繰り返すと `{}` |

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
`$json.object` は child を空へ戻します。parent と child が同じ別名でも、変更前の内容を有限の値として取り込んでからフィールドを追加するので循環を作りません。
初版は object・string・整数・boolean が対象です。配列・浮動小数点・null の値 API は未実装です。

`just test-stdlib` で Wasmtime と Core / Component の実行を確認します。ブラウザは `just test-e2e` に含まれます。
`just stdlib-capture DIRECTORY` で3アプリの生成物と入力・期待値・ソースのハッシュを固定し、変更後に別名で capture します。
`just stdlib-compare BEFORE AFTER` は固定した Core＋JSPI 成果物を交互に測定し、各呼出しの出力・入力終了・ABI の未解放数0を照合します。
