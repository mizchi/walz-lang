# `wlt:std` と複数束縛

WLT の基本型と標準関数は、通常の静的 import と同じ名前解決・別名・可視性の規則で参照する。
公開名・完全な関数型・型制約・effect は [生成した標準 API 一覧](stdlib-api.md) にまとめている。
汎用のtextキー辞書と不変listを作るbuilderは [コレクション](collections.md) を参照。

```wat
(module
  (%import "wlt:std"
    (type $text)
    (type $option)
    (type $result $Outcome)
    (type $future)
    (func $string.u64 $format))
  (func $main
    (export "main")
    (result (%inst $Outcome $text i32))
    (%effects reads writes allocates suspends)
    (%let
      (($pending (%inst $future i64) (%future.ready i64 (i64.const 42)))
       ($number (%future.await i64 (%move $pending)))
       ($label (call $format (local.get $number))))
      (%result.ok (local.get $label))))
)
```

`wlt:std` はコンパイラに同梱したモジュールで、ファイルパスや Wasm のホスト import ではない。文字列の check/compile API でも、ファイル経由でも同じものを解決する。解決時のネットワーク・ファイル探索・実行時のモジュール初期化は追加しない。

| import する型 | 使い方 |
| --- | --- |
| `$text` / `$bytes` / `$unit` | 型引数なし。その名前を型の位置に書く |
| `$option` / `$list` | `(%inst $option T)` / `(%inst $list T)` |
| `$result` / `$pair` | `(%inst $result T E)` / `(%inst $pair T U)` |
| `$own` / `$future` / `$stream` | `(%inst $own T)` / `(%inst $future T)` / `(%inst $stream T)` |
| `$resource` / `$borrow` | リソースの nominal 型 R に対して `(%inst $resource $R)` / `(%inst $borrow $R)` |

これらは既存の検査済み基底型への透明な alias。型引数を取るものも利用者定義の汎用 alias と同じ単相化を使い、新しい nominal identity や追加ランタイムを作らない。`i32`・`i64`・`f32`・`f64`・`ref` は Wasm の構文としてそのまま使う。

標準関数は `(func $string.u64)`、`(func $list.map)`、`(func $builder.new)` などを選んで import する。任意のローカル名へ変更できる。名前を取り込まない関数は呼出し元のスコープに現れない。依存する標準モジュールは一度だけ展開し、通常の型・effect・所有権・opaque 検査と DCE を適用する。builder や JSON 値の内部表現は公開しない。

同じスコープで同じ標準名と別名の組を繰り返して import した場合は、1つの束縛を共有する。これは `%include` で自己完結した宣言群を挿入する場合にも適用する。別の標準名を同じローカル名に割り当てたり、同名のローカル宣言を追加したりすれば衝突として拒否する。

`%text.lit`、`%result.ok`、`%option.some`、`%future.ready` / `%future.await` / `%future.close` などの固定構文はコンパイラの値操作。型の参照には import した型名を使う。`(result (%inst $result T E))` の外側は WAT の結果宣言で、内側が1つの Result 値型を表す。

## 複数束縛の `%let`

```text
(%let (($a [T] initial-a)
       ($b [U] initial-b)
       ...)
  body)
```

各初期値を左から右に一度だけ評価する。`initial-b` は `$a` を参照でき、`initial-a` は後続の `$b` を参照できない。同名で再束縛する場合、その初期値では以前の束縛が見え、新しい束縛は評価後から見える。すべての名前は body 内に有効で、式の外には出ない。

型注釈は束縛ごとに省略可能。不変性、初期値の型適合、effect、所有値の move と消費義務は単独の `%let` と同じ。初期値が trap・throw・中断を起こす場合も、ネストした `%let` と同じ順序と既存の終了処理に従う。Wasm の複数戻り値を束縛する構文ではない。pair の分解は `(%let ($first $second) subject body)` を使う。

`just test-standard` で、標準型・関数・別名・可視性と、Core＋JSPI / Component の Future・Result・Option ABI を確認する。
