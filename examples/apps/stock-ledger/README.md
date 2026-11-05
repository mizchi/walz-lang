# 在庫台帳での改善確認

既存のログ集計と違う処理を、新しいライブラリ・マクロ・opaque 型を使って実装した。
Unicode のキーを持つ在庫を更新し、操作結果を JSON Lines へ逐次出力する。パース・ハッシュ表・検査・JSON の組立ては wlt で実行する。
初回実装では新しい言語構文、コンパイラ命令、ランタイム API を追加しなかった。
現在の main は型引数の省略形を使う。[行数を返す版](main-counted.wlt) は型 alias と pair の分解 let も使い、行処理を `$advance` に分けている。
[文法改良後の評価](../../../docs/readability/implemented/REPORT.md) に変更と検証をまとめた。

## 実行

`wlt-lang` を作業ディレクトリにする。

```sh
just build-stock-ledger
just stock-ledger                     # sample.tsv の結果を出力
just stock-ledger input.tsv
node runtime/text-cli.mjs target/stock-ledger < input.tsv > events.jsonl
just test-stock-ledger
just audit-stock-ledger
```

入力は次の形式。区切りは TAB、コマンドは大文字に固定する。

```text
ADD<TAB>品目名<TAB>数量
TAKE<TAB>品目名<TAB>数量
SHOW<TAB>品目名
```

品目名は非空・最大256 UTF-8 byte。バイト列で完全一致させ、引用符・backslash・NUL・日本語・絵文字も扱える。
数量は0..4294967295の ASCII 十進数で、先頭の0を許す。内部の在庫数量は符号なし64 bitとして演算する。
ADD は数量を加え、TAKE は減らし、SHOW は現在値を返す。未登録の品目は0。ADD 0は品目を登録し、未登録への TAKE 0と SHOW は登録しない。
空行と先頭 `#` の行は読み飛ばす。構文不正・在庫不足・数量の桁あふれ・品目数の上限はエラーレコードを出し、次の行へ進む。

出力例：

```json
{"op":"ADD","key":"日本語🦀","ok":true,"quantity":10}
{"op":"TAKE","key":"日本語🦀","ok":true,"quantity":7}
{"op":"TAKE","key":"日本語🦀","ok":false,"error":"insufficient-stock"}
```

サンプル全体の期待値は [expected.json](expected.json)。成功した読取りでも業務エラーが含まれることがあり、CLI は正常な EOF なら0で終了する。
UTF-8 不正・読書きの失敗は main の Result err で全体を終了し、CLI は1を返す。取消しの終了確認は既存の task scope を使う。
書込み途中の失敗では部分的な出力が残り得る。在庫不足・桁あふれ・容量超過では在庫を変更しない。

これは1回の実行中に状態を持つ台帳で、永続 DB は使わない。64バケット・最大4096品目の具体的な実装とした。
GC に保持する状態は品目数に比例する。JSON の u64 数値を JavaScript の通常の JSON.parse で読む場合、2^53-1を超える整数の精度には注意が必要。

## 同じ処理を2通りに記述

- [main.wlt](main.wlt)：result.let と stream.fold-result を使う。
- [main-explicit.wlt](main-explicit.wlt)：同じ処理を loop / match / close で書く。
- [main-counted.wlt](main-counted.wlt)：出力は同じで、成功値を配送行数に変える。空行・コメント・I/O が成功した業務エラー行も数える。

行数版は `just build-core-js examples/apps/stock-ledger/main-counted.wlt target/counted-ledger` でビルドできる。
text-cli は成功 payload を表示しない。数値の確認方法は [チートシート](../../../CHEATSHEET.md#確認コマンド) の runTextProgram の例を使う。

両版は以下を共有し、比較対象を main の制御フローに絞った。

| ファイル | 責務 |
|---|---|
| [parse.wlt](parse.wlt) | TSV、コマンド enum、数量とキーの検査 |
| [stock.wlt](stock.wlt) | opaque State、GC の連鎖ハッシュ表、数量変更と業務エラー |
| [report.wlt](report.wlt) | 入力から得た文字列を JSON エスケープし、結果を出力 |
| [program.wlt](program.wlt) | 業務 Result と I/O Result の接続、共有ライブラリの読込み |

`Aa` と `BB` は実際に同じハッシュ値になるため、衝突時も別々の在庫を保持できることを検査する。
再利用した `$string.json`・`$string.concat`・`$string.u64` により、可変長の入力キーと数量を手動のバイト列生成なしで出力できた。

## 意図どおり改善した部分

以下の数値と実行コストは、型引数を明示していた初回のスナップショットの記録。現在の省略形や行数版の計測値ではない。
初回の同じ formatter を適用した後の main の比較：

| 指標 | 手書き版 | マクロ版 |
|---|---:|---:|
| 行数 | 70 | 31 |
| トークン数 | 352 | 126 |
| S式数 | 90 | 31 |
| 明示的な match | 4 | 0 |
| 明示的な stream.close | 3 | 0 |

これは main の比較で、ハッシュ表・パーサ・共通ライブラリを含むプログラム全体の削減率ではない。
終了処理は単に削除したのではなく、マクロの展開後に型・effect・所有権検査を受ける。
読取り／書込みの失敗、両方の待機中の取消しを両版で実行し、終了後の端点登録と ABI バッファは0だった。

在庫状態への外部からの struct.set と、更新関数を pure な関数から呼ぶ試みはコンパイル時に拒否された。
計算部分の stock.add / stock.take / parse.command は host と suspends を持たず、app.process だけが I/O を接続する。
formatter は両版・各モジュールに適用し、fmt-check と実行テストを通した。

## 動作の照合

Native の7テストでは、衝突、日本語キー、在庫不足、容量上限、ゼロ操作、u64 の桁あふれ、パーサを確認した。
Node は Core と Component のそれぞれで3版を構築し、3,000操作を独立した JavaScript Map / BigInt の実装と照合する。行数版は成功 payload も配送行数と照合する。
追加で、空入力、u32 最大値、先頭の0、引用符・backslash・NUL、256/257 byte のキー、無効なフィールド、I/O 失敗と取消しを検査する。
テスト用の小さい容量と u64 最大値を使った呼出し元でも、quantity-overflow / capacity の JSON を確認する。
Chromium でも両バックエンドの実際の入力・更新・JSON 出力・解放を実行する。

## 実行コストはほぼ同等、一時 Result は課題

2026-11-04、Apple M5 / macOS arm64、Node v24.21.0。
Core＋JSPI で3,000行をメモリ上から配送し、4回 warmup 後に順番を入れ替えて9回ずつ測った。
Wasm コンパイル・インスタンス化・入力生成は除外し、main 完了までを測る。出力は独立した期待値から得た SHA-256 と毎回照合する。

| 条件 | 手書き版・再計測 | マクロ版・再計測 |
|---|---:|---:|
| ASCII キー | 12.34 ms | 12.23 ms |
| 日本語・引用符・backslash・絵文字のキー | 13.19 ms | 13.19 ms |

初回は ASCII が12.15/12.05 ms、日本語が12.48/13.15 msで、差は安定しなかった。今回の結果から速度改善は主張しない。
全ケースで出力2,842回、JSPI read 5,843回は両版で同じ。終了後の線形 ABI メモリは64 KiB、未解放ブロックは0だった。
この値は Wasm GC ヒープ全体の使用量ではない。ファイル I/O の throughput も測っていない。

生成した Core Wasm は手書き版11,351 byte、マクロ版11,617 byte。
展開後の main の Match は4→6、VariantNew は3→7箇所になった。これらは静的な IR の構築箇所数で、実行時の割当て回数ではない。
ソースの繰返しは減ったが、一時 Result の生成・再分岐が残ることを確認できた。即座に match する値の簡約は、次の最適化候補とする。

## 残る書きにくさ

パーサでは TAB 探索、範囲の切出し、整数解析を依然として低水準のループで書いた。ハッシュ表も具体的な実装が必要だった。
次に足すなら文字列／bytes の検索・分割・整数解析と、型を固定したコレクションのライブラリが有用。
今回の新規アプリケーションは既存の仕様だけで成立したので、これらはまずライブラリで整理できる。

`target/stock-ledger-audit/report.json` に再計測、`report-initial.json` に初回の測定を保存した。
両版の生成物、ir.json、ソースと共有ライブラリも同じディレクトリに保存する。
