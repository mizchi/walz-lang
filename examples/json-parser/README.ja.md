# Walz の JSON パーサーを動かす

[English](README.md) | 日本語

JSON の読み取り、GC 上の AST 構築、シリアライズはすべて Walz で実行する。
実装は [walz/core/json-value.walz](../../walz/core/json-value.walz)、入口は [main.walz](main.walz)。
Node 側の [CLI](../../runtime/json-parser.mjs) は UTF-8 の入出力と診断表示を担当し、`JSON.parse` は呼ばない。

```sh
just build-json-parser
just json-parser < examples/json-parser/sample.json
```

出力:

```json
{"answer":42,"items":[null,true,"日本語🦀"],"large":9007199254740993,"decimal":1.2300e-2}
```

ファイルを引数に渡す場合:

```sh
node runtime/json-parser.mjs target/json-parser examples/json-parser/sample.json
```

空白を除去するが、キーの順番・重複と数値の字句は保持する。`9007199254740993` は浮動小数点に変換しない。
パーサーは null / boolean / number / string / array / object と Unicode escape、UTF-16 surrogate pair を扱う。
対応のない surrogate、末尾カンマ、余計な文字、壊れた数値・escape は拒否する。

`normalize(text)` は `result<text,i32>`。err は0始まりの UTF-8 バイト位置。
CLI は不正なJSONで終了コード1、標準出力は空にし、標準エラーへ位置を出す。
表示の行・列は1始まりで、列は Unicode scalar 数。CRLF は1改行として数える。
正常時は0。入力は1文書で、複数行でもよい。JSON Lines を個別に読むインターフェースではない。

```text
JSON parse error at byte 3 (line 1, column 4)
```

上は `[1,]` の例。入力上限は65,536 bytes、array/object のネストは128。
空のcontainerも深さへ数える。CLI は上限を超えた時点で読み取りを止め、UTF-8の不正も拒否する。
BOMは黙って除去せず、パーサーが不正な先頭文字として扱う。

## 実装の読み方

- `$parse-value`: 先頭byteで値を振り分ける。深さと各値の入口を早期 `return` で表す。
- `$parse-number`: 整数部・小数部・指数部を `@result.do` で順に読む。成功位置を次の段階へ渡す。
- `$string-end` / `$parse-string`: 通常の文字列をまとめて走査し、エスケープがある場合だけbuilderを使う。
- `$escape-unicode`: Unicode escapeをUTF-8へ変換する。
- `$parse-array` / `$parse-object`: 兄弟要素はloopで読む。再帰はネストした値を読むときだけ使う。
- `$parse`: 最後の空白まで読んで入力が尽きたことを検査し、ASTを返す。

ASTはopaqueなvariantと不変のGC listで、ホストのobjectや独自ヒープを使わない。
`main.walz` の `$answer` は、公開された `get`・`as-number` と整数解析だけでASTを調べる例。

```sh
just run examples/json-parser/main.walz  # 42
just test-json
```

Wasmtimeで境界とUnicodeを実行し、NodeのCore＋JSPI / Componentでは独立したJSONパーサーと照合する。
CLIの標準入力・ファイル・エラー表示、各呼出し後の未解放ABIバッファ数0、Chromiumの両backendも検査する。
EOFまでの入力を一度に読む再帰下降パーサーであり、増分のストリーミングパーサーではない。


速度比較は [JSONパーサーのベンチマーク](../../docs/json-parser-bench.md) を参照。JSON.parse、serde_json releaseと、解析本体・入力ABI・再シリアライズを分けて測定できる。
改善した箇所と採用しなかったSIMD方式は [チューニング結果](../../docs/json-parser-tuning.md) に記録している。
