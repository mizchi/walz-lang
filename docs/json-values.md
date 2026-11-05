# JSON の値と parser

`stdlib/json-value.wlt` は言語のプリミティブを増やさず、opaque な不変の JSON 値を実装する。
named import で使う関数だけを選べる。入口の例は `tests/component-fixtures/json-value.wlt`。

| API | 契約 |
| --- | --- |
| `parse(text)` | `result<(ref Value), i32>`。err は UTF-8 のバイト位置 |
| `stringify(Value)` | JSON text。数値の字句を保持する |
| `null/boolean/string/array/object` | 不変値を作る。array/object は空 |
| `number(text)` | JSON 数値として検査して作る。結果は Result |
| `prepend(array, item)` | 先頭へ追加した新しい配列を Result で返す |
| `set(object, key, item)` | 末尾へ追加した新しい object を Result で返す |
| `get/at` | key／添字の値を Option で返す |
| `as-string/as-number/as-boolean` | 対応する payload を Option で返す |
| `kind` | null/boolean/number/string/array/object の enum |

小数と指数を含む数値は text のまま保持する。`9007199254740993` のような整数も丸めない。
整数として計算する場合は `string.parse-s64/parse-u64` と組み合わせる。
binary float の最短文字列表現や、十進から浮動小数点への正確な丸めはこの API の対象外。

object は重複 key を保持し、`get` は最後の値を返す。set は既存の値を書き換えない。
配列の兄弟要素と object の field はループで処理する。再帰は値のネストに限る。
parser は入力 65,536 bytes、ネスト128まで。JSON の4種類の空白を認め、後続のごみ、
制御文字、壊れた数値、末尾カンマ、壊れた escape を拒否する。
Wasm UTF-8 の意味に合わせ、対応しない UTF-16 surrogate escape も拒否する。

既存の object builder にも `json.null`、`json.number`、`json.value` を追加した。
number は検査に失敗した場合 builder を変更しない。value は配列も含む任意の JSON 値を書ける。
数値を扱うホスト依存ランタイムは加えていない。

実装で共通化候補になったのは要素 list と field list の反転だが、payload の形が異なる
2つの短いループに留まる。単相化ジェネリクスは今回追加せず、重複が増えた段階で再評価する。
