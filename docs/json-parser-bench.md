# JSONパーサーの速度比較

Node v24.21.0 / V8 13.6 / Apple M5 arm64、Rust 1.98.0、serde_json 1.0.154のreleaseビルドで測定した。
9種類の入力を固定し、実行順を1つずつ回転して9回測定した。以下は1文書あたりの中央値、単位はµs。

## 入力が既に各処理系にある状態での解析

| 入力 | UTF-8 bytes | JSON.parse | serde_json | WLT / Wasm GC | WLTの時間 / Node |
|---|---:|---:|---:|---:|---:|
| small-object | 56 | 0.16 | 0.19 | 0.37 | 2.27倍 |
| records-128 | 9,633 | 14.81 | 26.20 | 52.74 | 3.56倍 |
| records-512 | 40,161 | 58.91 | 106.86 | 217.48 | 3.69倍 |
| numbers-4096 | 25,747 | 40.47 | 47.70 | 132.36 | 3.27倍 |
| wide-object | 24,891 | 121.22 | 181.06 | 129.89 | 1.07倍 |
| ascii-string | 48,002 | 12.09 | 3.17 | 128.74 | 10.65倍 |
| unicode-escapes | 4,107 | 2.98 | 3.76 | 19.04 | 6.38倍 |
| nested-64 | 135 | 1.62 | 1.68 | 1.27 | 0.78倍 |
| whitespace | 60,056 | 14.17 | 21.30 | 55.00 | 3.88倍 |

WLTは多くの入力でJSON.parseより時間がかかる。レコード配列は約3.6–3.7倍、数値配列は約3.3倍、長いASCII文字列は約10.6倍。
2,000フィールドのobjectでは約1.07倍、64段のネストでは約0.78倍だった。入力の形とASTの表現によって差が大きい。
この測定だけでは、個々の関数が遅い原因や、特定の最適化の効果までは断定しない。

## JSから実際に呼ぶ場合

Core＋JSPI / Componentについて、解析して成功状態だけを返すAPIと、解析して再シリアライズするAPIを別に測定した。
入力長だけを返す同じ型のAPIを対照として置き、入力コピー・UTF-8変換・呼出しの費用も記録した。対照の時間を引き算した値は報告しない。

約40KBのレコード配列では次の通り。

| 処理 | Node | serde_json release | Core＋JSPI | Component |
|---|---:|---:|---:|---:|
| 解析＋再シリアライズ | 102.63 | 137.93 | 512.14 | 543.31 |
| 入力ABI＋解析、成功状態を返す | — | — | 281.65 | 311.23 |
| 入力ABI＋入力長だけを返す | — | — | 74.17 | 97.15 |

全条件の時間、生サンプル、最小・最大、MiB/s、出力byte数は [固定した結果](../benchmarks/json-parser/20261106-r2/report.json)、読みやすい表は [report.md](../benchmarks/json-parser/20261106-r2/report.md) に保存した。
解析状態だけのAPIはASTを外へ返さないため、ASTを返す最初の表とは仕事が異なる。

## 条件と検査

- 原文・Wasm・JS glue・native参照バイナリ・ソース・入力を計測前にcaptureし、SHA-256を検査してから実行する。過去のcaptureとreportは上書きしない。
- コンパイル・instantiate・ファイルI/O・原文をGCへ置く準備・結果の照合は時間へ含めない。測定プロセスはビルドツールを読み込まない。
- 実装ごとに約30ms以上になる反復数へ調整し、選んだバッチで3回warmupする。各測定バッチ前の明示GCは時間外、バッチ途中の自動GCは時間内。
- WLTの解析本体はJSへ実際のGC ASTを返す。時間外で全文の再シリアライズとFNV-1a照合を行い、不要結果として消されることと、誤った結果の計測を防ぐ。
- NodeはJSON.parseが返すJS object、Rustは所有するserde_json::Value、WLTはGC variantとlistを作る。Rustは各反復のAST破棄も時間内であり、メモリ管理とデータ表現は同一ではない。
- RustはUTF-8文字列を常駐させたnativeプロセスで実行する。子プロセスの起動と入力読み込みは時間外で、各プロセス内で32回の解析を行ってから測る。JS FFIは含まない。
- Core/Componentの全バッチ後に、ABIアロケータの未解放数が0であることを照合した。ランタイムの計測用書き換えは行っていない。

速度比較の入力は、安全な範囲の整数、有限の小数、重複のない整列済みキーに限定した。
WLTは数値の字句と重複キーを保持するが、Nodeやserde_jsonの既定のValue/Mapは同じ表現ではない。
数値字句の保存を示す追加検査では、9007199254740993はWLTでそのまま、Nodeでは9007199254740992として再出力された。この入力は速度順位には使っていない。

Unicodeの改行は、WLTが`\u000a`、Nodeとserde_jsonが`\n`で出力する。JSONの値の一致を検査し、文字列表記の一致を要求しない。
このケースの出力はWLTが2,443 bytes、Node/serde_jsonが1,931 bytesで、この出力サイズ差も再シリアライズの時間に含まれる。
入力上限65,536 bytes、ネスト128というWLT側の制限内での比較であり、エラー入力・起動時間・streaming・ブラウザの速度比較ではない。

## 再実行

```sh
just test-json-bench
just json-bench-capture target/json-parser-bench/new-run
just json-bench target/json-parser-bench/new-run target/json-parser-bench/new-results 9
```

今回の実行成果物は`target/json-parser-bench/20261106-r2/`、結果は`target/json-parser-bench/20261106-r2-results/`。
同じcaptureを再測定する場合は、2番目のコマンドを省略し、結果に新しいディレクトリを指定する。
VCSには結果・captureメタデータ・入力とrunnerを [benchmarks/json-parser/20261106-r2](../benchmarks/json-parser/20261106-r2) に保存した。実行バイナリはローカルのtarget以下に残る。
