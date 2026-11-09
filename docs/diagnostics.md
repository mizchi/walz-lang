# 診断のデータ契約

`wlt::Error::diagnostic()` は `&wlt::diagnostic::Diagnostic` を返す。
診断データとテキスト描画を分離し、呼び出し側で表示文を解析せずに位置や分類を取得できる。
`serde_json::to_value(error.diagnostic())` で JSON にもできる。

| フィールド | 意味 |
|---|---|
| `code` | 機械処理用の分類 |
| `message` | 主エラーの説明 |
| `primary` | 問題のソース範囲。外部ツール等の失敗では `None` |
| `related` | 宣言、先行する move、effect の原因などの位置と説明 |
| `notes` | 位置を持たない補足 |
| `help` | 名前候補や修正方針。自動修正ではない |

各ラベルは `span`、`location`、`message` を持つ。
`span` は UTF-8 byte offset の半開区間、`location.line/column` は1始まり。
列は Unicode scalar の個数で数え、バイト数や画面上の文字幅では数えない。

文字列 API の `check` / `compile` が返した位置は、その文字列に対するもので、`location.path` は `None`。
include を使う場合は `source::load(path)` の展開後テキストを検査し、エラーを `loaded.diagnostic(error)` に渡す。
主エラーとすべての関連位置を、それぞれ元ファイルの範囲・行・列へ変換する。
変換済みのラベルの `span` は `location.path` のファイルに対するものになる。
include 境界をまたぐ範囲は、開始ファイル内の連続する部分までに制限する。
CLI とテストランナーもこの変換を使う。

| code | 診断 |
|---|---|
| `E_CALL_ARITY` | 呼び出し先と引数数。通常の call では関数名を数えない |
| `E_UNKNOWN_LOCAL` | 現在のスコープにない名前 |
| `E_TYPE_MISMATCH` | 期待する型と実際の型 |
| `E_OWNERSHIP_MOVED` | 消費済み、または利用不能な所有値の再使用 |
| `E_OWNERSHIP` | 所有権契約の状態不一致や未消費値 |
| `E_EFFECT_CONTRACT` | 宣言した effect の上限を超える処理 |
| `E_OPAQUE_REPRESENTATION` | 所有する ファイル の外からの表現アクセス |
| `E_ALIAS_CYCLE` | 透明な型 alias の参照循環。順序付き経路と各参照位置を表示 |
| `E_GENERIC_INFERENCE` | 汎用呼び出しの型引数が曖昧、未確定、または衝突 |
| `E_MACRO` / `E_MACRO_UNKNOWN` | マクロの宣言・テンプレートの構文、または未定義のマクロ |
| `E_MACRO_MATCH` | 構文・型制約に一致するルールがない |
| `E_MACRO_TYPE` / `E_MACRO_INFERENCE` | 型問い合わせの対象不一致、または必要な具体型・期待型がない |
| `E_MACRO_BINDING` / `E_MACRO_HYGIENE` | 宣言した束縛関係・重複・委譲、またはテンプレート内の自由なローカル参照の違反 |
| `E_MACRO_LIMIT` | 展開数・サイズ・深さ・検査量の上限 |
| `E_CHECK` | その他の構文・名前解決・検査エラー |
| `E_TOOL` | ソース範囲を持たないツール・入出力等の失敗 |

`%closure.call` の個数には、明示的に渡す environment を含む。
その場合は environment 1個と通常の引数を分けて補足する。capture の個数は加算しない。
名前候補は現在のスコープだけから最大3件を選ぶ。capture の説明は closure 内で候補がない場合に限る。
近い名前がない場合は、利用可能な束縛とその型・宣言位置を最大4件示す。置換先と断定せず、既存の束縛を意図した場合の案内として表示する。

型の期待値には数値演算の operand、呼び出し先の parameter、宣言した result の根拠を付ける。
失敗した operand と周辺の期待型が同じ数値型なら、その型に対応する同種の演算と定数型の確認を条件付きで案内する。実在する命令で型・引数数が合う場合に限り、暗黙の変換は行わない。
内側の式が既に原因を説明している場合、外側の result や call を原因として追加しない。
所有権診断は束縛の宣言と先行する move を示し、分岐では到達可能な複数の位置を最大3件表示する。
effect 診断は宣言と必要な作用を並べ、原因の操作まで呼び出しを最大8段追う。catch で処理済みの throw は原因にしない。
これらの補足は診断用データであり、所有権の proof contract や effect の許可判定には使わない。

マクロでは入力の位置と展開ルールの位置を区別する。末尾以外の `...` はそのトークンを指し、末尾反復や再帰ルールを案内する。委譲する本体を組み直した場合は、本体全体を転送し、ラッパーを委譲先のスコープ内へ移すよう案内する。
`E_UNKNOWN_LOCAL` が捕捉した初期化式内にあり、同名の呼び出し側の束縛を確認できた場合は、初期化式が caller のスコープを使うことを補足する。無関係な未定義名にこの説明を付けたり、暗黙の捕捉でコードを受理したりはしない。[対応範囲](user-macros.md#hygiene-evaluation-and-intentional-bindings)も参照。

回帰テストは `cargo test --locked --test diagnostics`。
実際のメッセージの読解・修正品質は [評価ループ](../evals/diagnostics/README.md) で別に評価する。
