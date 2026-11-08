# データ処理と厳密な64bit整数のJS置換候補

調査日: 2026-11-08。外部資料は公式ドキュメント・実装元READMEのみ。以下の候補の対JS速度は未測定で、達成済みとは扱わない。

## どの処理ならWLTの性質を性能へ変えられるか

### Takeaway

有望なのは、固定幅i64の計算を一括して行うAPIと、UTF-8から選択・検証・集計までをWasm内で完結するAPI。既に存在するJSオブジェクト1個をWasmへ送り、検証結果だけ返すAPIは、現状の転送費用に見合う根拠が薄い。

### Cited Findings

- 数値listのCore-JS ABIは `i64 -> BigInt64Array`、nominal u64は `BigUint64Array` の一括転送に対応する。これはGC listへの変換が消える意味ではない。転送改善後でも単発の合計・内積・bitmap処理は既存JS対照より遅かった。— [実装](../../src/component/core_js/values.rs:13)、[保存された計測の説明](../../docs/simd-kernels.md:95)
- WLTの十進整数読取には、乗算前に上限を検査する `$bytes.parse-u64` と、符号付き64bit範囲を検査する `$string.parse-s64` がある。— [bytes](../../stdlib/bytes.wlt:126)、[string](../../stdlib/text.wlt:65)
- JS側にも厳密整数の経路はある。PapaParseはdynamicTypingで安全範囲を超える数値をNumberへ変換せず文字列のまま残す。`@streamparser/json` は既定のNumber変換を行う `parseNumber` を差し替えられる。protobuf.jsは `Long` / `BigInt` / 十進文字列との変換を提供する。— [PapaParse](https://www.papaparse.com/docs)、[@streamparser/json](https://github.com/juanjoDiaz/streamparser-json/blob/main/packages/plainjs/README.md)、[protobuf.js](https://github.com/protobufjs/protobuf.js/blob/master/README.md)
- JSON.parseのreviverにはprimitiveの原文を渡すcontext.sourceがあり、丸め済みNumberからではなく原文からBigIntへ変換できる。ローカルNode v24.21.0でも `9007199254740993` が保持されることを読取のみのプローブで確認した。— [ECMAScript JSON.parse](https://tc39.es/ecma262/multipage/structured-data.html#sec-json.parse)

### Inferences

候補は以下の順に、まず新しい言語機能やABIなしで試せる。

| 候補 | 置換対象となるアプリ側の処理 | WLTで省ける可能性がある費用 | 現状で始める実装 | 追加が必要なもの |
|---|---|---|---|---|
| 厳密i64のバッチ集計 | BigInt / long.js の時刻差分、カウンタ差分、固定小数点の整数係数、条件別合計 | 演算ごとの汎用数値処理、同じ入力の再読込 | `BigInt64Array -> list<i64> -> 複数の計算 -> 小さな結果` を1呼出し内で行う | 処理ライブラリだけ。加算・乗算・overflow方針を明示する。u64境界ならnominal u64を使う |
| NDJSONの選択・検証・集計 | `JSON.parse + Ajv/Zod/typia + filter/groupBy/reduce`、streamparserのonValue処理 | 不要フィールドのデコード・AST・中間配列・行ごとのFFI | `analyze(bytes)` が多数行を読み、必要な2〜4項目だけ値化し、集計結果だけ返す | 選択的scanner/consumer。最初は全bufferを1回で処理、必要になってからchunk継続状態を追加 |
| CSV/TSVの選択・整数変換・集計 | PapaParse / csv-parse と後続集計 | 不要列の文字列化、汎用cast、行オブジェクトと中間配列 | `analyzeCsv(bytes)`。固定dialect・固定schemaで選択列の型検査と集計を統合 | 引用符・CRLF・escape・列数を扱うscanner。既存TSV実例は出発点で、CSV互換を既に実装したとは言わない |
| バイナリtelemetryの選択・集計 | protobuf.jsのdecode後に繰り返す検証と集計 | 不要フィールドのオブジェクト化、厳密64bit値の境界変換 | 多数のframed messageを受け、必要フィールドだけデコードして集計 | wire readerとschema専用consumer。WIT bindingはProtobuf codecではなく、別ライブラリ実装が必要 |

「固定幅なので必ず高速」「BigIntは毎回必ずheap確保」は未確認のエンジン実装仮説なので主張しない。64bit以内に制限できる用途に絞り、checked / wrapping / saturationの同じ規則をJS対照にも実装する。金融一般の任意精度decimalをi64へ置き換える案ではない。

全件をJSへ返すdecode単独より、結果の小さい集計・検索・projectionを優先する。APIをアプリ用途へ絞るため、PapaParseやprotobuf.jsの全公開APIと互換なdrop-in replacementとは区別する。

### Gaps

- 4候補とも同じ仕事をする最新JSライブラリへのE2E勝利は未測定。どの項目数・chunkサイズで転送費用を償却できるかも未確定。
- native i64とV8 BigIntの実際の命令・allocation差は、対象Node/Chromiumで計測またはprofileが必要。

## 強い比較対象と意味の一致は何か

### Takeaway

JS側も既にコード生成・streaming・projectionを持つ。WLTの速さを示すには、推奨設定の最新ライブラリと、同じ型付き処理を融合した手書きJSに勝つ必要がある。

### Cited Findings

- Ajvはcompile-onceで再利用する設計で、standalone生成も提供する。JTDの `compileParser` は解析中に検証し、schemaにないプロパティを既定で拒否する。公式性能説明は著者側の主張であり、WLT比較の実測ではない。— [Ajv管理規約](https://ajv.js.org/guide/managing-schemas.html)、[JTD parser](https://ajv.js.org/api.html#ajv-compileparser-schema-object-json-string-any)
- Zod 4.5（2026-08-28）は `z.compile(schema)` を追加している。未コンパイルZodだけを対照にしてはならない。typiaのjson parse系はnative JSON.parse成功後に生成validatorを実行する。— [Zod公式](https://zod.dev/blog/zod-4-5)、[typia公式](https://typia.io/docs/json/parse/)
- PapaParseにはchunk/stepと無引用符向けfastModeがある。csv-parseはcolumns配列のfalse/nullで不要な出力列を省き、encoding:null/falseでraw buffer出力もできる。— [PapaParse設定](https://www.papaparse.com/docs)、[csv-parse設定](https://csv.js.org/parse/options/)
- `@streamparser/json` はUTF-8 bytesを直接受け、paths、keepStack:false、separator設定で不要な保持を避けられる。protobuf.jsはreflection/staticとも同じコード生成backendと調整済みreader/writerを使う。— [streamparser README](https://github.com/juanjoDiaz/streamparser-json/blob/main/packages/plainjs/README.md)、[protobuf.js README](https://github.com/protobufjs/protobuf.js/blob/master/README.md)

### Inferences

- **整数kernel**: 手書きBigInt loop、BigInt64Array処理、適切なLong実装を比較。safe Numberへ落とす対照は、全域で厳密64bitを要求する場合には別条件にする。
- **NDJSON**: native JSON.parse＋キャッシュ済みAjv/compiled Zod/生成typia＋単一loop、Ajv JTD parse＋集計、streamparserのpaths/keepStack:false＋集計、可能なら同じ選択scannerの手書きJS。JS側に無駄なmap/filter配列を強制しない。
- **CSV**: PapaのfastModeが可能な無引用符データと、正規の引用符データを分ける。csv-parseのprojection、stream、Buffer入力を使う。dialect、BOM、末尾空行、multiline quote、未知列、重複header、空とnullの扱いを固定する。
- **JSONの省略**: 不要フィールドもJSON構文・Unicode escape・UTF-8・深さ・重複key規則は検査する。「結果に使わないから無効なJSONを許す」を速度差へ混ぜない。JTDと通常JSON Schemaの表現力や追加プロパティ規則は同一ではないので、共通schemaへ絞る。
- **エラー**: first-errorのみとall-errors列挙を別比較にする。row reject継続と全処理停止も区別。桁あふれした行で状態を変更するか否か、重複JSON keyのlast-winsか拒否か、浮動小数点の許容、UTF-16 lone surrogateに対する方針も揃える。

### Gaps

- JSライブラリの上記全機能を同時に使う現実的な最適設定は、各固定workloadを実装して検証する必要がある。著者の倍率を別環境へ転用しない。
- schema専用WLT readerを作らない段階では、融合parse/validationの同等性能を期待する根拠はない。

## 現状の制限と最初の実験は何か

### Takeaway

最初は1呼出し内のバッチで検証し、境界コピーを含む速度差が出るか判定する。現行JSON parserを大入力へそのまま使う計画と、ゲストのviewをABI越しzero-copyとする説明は成立しない。

### Cited Findings

- 現行 `$json.parse` は65,536 bytesを超える入力を拒否し、container深さ128を上限とする。GC ASTを作る実装である。— [入力上限のコード](../../stdlib/json-value.wlt:723)、[深さのコード](../../stdlib/json-value.wlt:661)
- 保存された2026-11-06のM5/Node24測定で、40KBレコードのCore-JS解析＋再出力はチューニング後420.74µs。SIMD文字列走査は短いkeyの多いレコードで遅くなり、GC bytesからv128へ16回のbyte読出しとlane挿入が残るため採用されていない。— [チューニング結果](../../docs/json-parser-tuning.md:29)
- bytes/string viewはゲスト内で元buffer＋rangeを共有する。小さなviewも元buffer全体を保持し、portable ABIへは値へコピーして渡す。— [viewの契約](../../docs/views.md:57)
- 過去のlog-stats測定では30,000行を処理するJSPI readが30,002回あり、1行単位の境界を大量に通っていた。これは古い記録で、現行batch機能の実測ではない。— [保存された計測](../../examples/apps/log-stats/README.md:74)
- V8はJSON.stringifyに専用fast path、SIMD/SWARによる文字列分類、同shape object向け高速化を実装し、一部をJSON.parseにも適用している。Node cryptoはOpenSSLを利用する機能を持つ。これらの成熟した組込みの単純な置換は最初の目標にしない。— [V8公式](https://v8.dev/blog/json-stringify)、[Node crypto](https://nodejs.org/api/crypto.html)

### Inferences

**最初の実験案（未実施）**:

1. 既存ABIのまま `analyze(bytes) -> {accepted,rejected,total,groups}` を作る。32項目中3項目を使用するNDJSONまたは固定CSVで、scanner/型検査/checked i64集計を同一loopへまとめる。最初はscalar実装とし、無理にGC bytesをSIMD化しない。
2. 入力は64KiB、1MiB、8MiB。現行value ABI上限を超えないサイズを使い、現行JSON AST parserはbaseline/小文書だけに使う。NDJSONの各行も64KiB以内なら既存parserと照合できるが、バッチ全体をそのparserへ渡せるわけではない。
3. 0/1/10%不正行、選択率0/1/50/100%、0/10/50%escape・quote、ASCII/日本語、低/高cardinalityのgroup、数値の2^53周辺とsigned64両端、overflowを独立に振る。chunk実装を加えるなら4/64/256KiB境界と分断UTF-8・分断escape・分断recordを含める。
4. E2Eは入力byte列から同一出力までのコピー・UTF-8処理・GC・境界・解放を含める。入力が最初からJS stringの条件も別測定し、WLTだけUTF-8準備を時間外にしない。Core-JS/Component、Node/Chromiumを分け、cold compile/instantiateとwarm throughputも別にする。
5. 推奨の探索打切り基準の例は「強いJS対照の最速実装に、主要サイズ2つ以上でE2E 2倍以上、95%区間が1を跨がず、小入力の不利/peak memory/cold latencyも開示」。これは達成値ではない。速度2倍未満でもmemory低減の価値は別評価できる。
6. group keyを保持するとき、入力chunkのviewをそのまま永久保持せず、初登場時に必要範囲だけ独立コピーする。そうしないと小さなkeyが多数のchunk全体を保持する。反復後にABI解放とGC到達性も検査する。

既存の言語・一括ABIで開始できるので、必要な追加はまず専用scannerと集計ライブラリ。複数呼出しにまたがる独自index handleやguest-defined resource exportは別のABI設計であり、このバッチ実験の前提にしない。

### Gaps

- 今回は新しい速度計測を行っていない。上記の候補・閾値・サイズは実験設計で、速度勝利の証拠ではない。
- 汎用JSON.parse、JSON.stringify、TextDecoder、native crypto全体を置き換えられる証拠はない。特定の選択・検証・集計の仕事を減らして勝つ仮説と区別する。
