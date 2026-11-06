# ストリーミングログ集計

ファイルまたは標準入力からログを読み、レベルごとの件数・合計時間・最大時間を JSON に出力する。
パース・集計・整数の文字列化・JSON の組立てはすべて wlt で実装した。ホストは既存の text I/O capability を接続するだけで、集計処理を持たない。
bytes.from-array、Result/Stream の限定マクロと opaque な型を使用する。ABI とタスクランタイムは既存のものを使う。

`wlt-lang` を作業ディレクトリとして実行する。

```sh
just build-log-stats
just log-stats                         # sample.tsv を集計
just log-stats /path/to/input.tsv
node runtime/text-cli.mjs target/log-stats < input.tsv > report.json
node runtime/text-cli.mjs target/log-stats --input input.tsv --output report.json
just test-log-stats
just analyze examples/apps/log-stats/main.wlt
just profile-log-stats
```

## 入出力の契約

入力の1行は `LEVEL<TAB>duration_ms<TAB>message`。

- LEVEL は大文字の INFO / WARN / ERROR。
- duration_ms は ASCII の十進数で、0..4294967295。空・符号・空白・桁あふれを拒否する。先頭の0は許す。
- 第2の TAB は必須。message は空でもよく、その後の TAB も内容として扱う。message の内容は集計状態に保存しない。
- 空行と、先頭が `#` の行は ignored。その他の構文不正は rejected として数え、処理を続ける。
- LF / CRLF、改行なしの最終行、分割された UTF-8 は既存の I/O 層で扱う。既定の行長上限は1 MiB。

サンプルの出力内容は [expected.json](expected.json)。空の入力でも、すべてのカウンタが0の同じスキーマを出す。
文字列ライブラリを使い、レポート全体を1行の JSON として出力する。
見やすく整形する場合は `just log-stats | jq .` などを使える。

件数と合計は符号なし64 bitとして扱い、桁あふれを検出したレコードは状態を変更せず、処理全体を `err(1001)` で終了する。
JSON には整数を十進表記で出す。JavaScript の通常の `JSON.parse` では、2^53-1を超える整数の精度は保証されない。
UTF-8 不正・I/O エラーは既存の Result エラーで終了する。正常な EOF まで集計できた場合にだけレポートを出す。
出力中に失敗した場合、途中まで書かれたレポートは残り得る。CLI の終了コードで成功を判定する。

## 構成と状態

初版の5ファイル・約260行から、共通ライブラリと限定マクロへ処理を移した。

| ファイル | 責務 | effect 上限 |
|---|---|---|
| [parse.wlt](parse.wlt) | TSV と u32 をパースし、skip / invalid / info / warn / error を返す | allocates |
| [stats.wlt](stats.wlt) / [stats-impl.wlt](stats-impl.wlt) | opaque な状態、件数・合計・最大と桁あふれ検査 | 初期化は allocates、更新は reads / writes |
| [decimal.wlt](decimal.wlt) | 共通の u64 変換への接続 | reads / writes / allocates |
| [report.wlt](report.wlt) | JSON の組立てと1回の非同期書込み | reads / writes / allocates / host / suspends |
| [main.wlt](main.wlt) | fold-result と result.let による反復とエラー伝播 | reads / writes / allocates / host / suspends |
| [stdlib/text.wlt](../../../stdlib/text.wlt) | 連結・符号付き／符号なし整数・JSON string | reads / writes / allocates |

文字列操作の writes は、関数内で新規作成する作業配列の変更を含む保守的な上限で、入力 text/bytes は不変。
集計状態のフィールド操作は、その型を宣言した ファイル の中だけで認める。
公開する Level は info/warn/error の enum で、配列添字のような任意の整数を受け取らない。
入力行の enum にはレベルと duration を直接格納し、別の Level オブジェクトを毎行作らない。

%use / %export で静的にリンクする。動的な関数探索やモジュールオブジェクトはない。
集計状態のサイズはレベル数に固定され、入力行数には比例しない。GC オブジェクトの回収は Wasm エンジンに委ねる。
Stream の所有権はマクロの展開後の loop/move/match で検査し、EOF・入力エラー・集計エラーの各経路で閉じる。
JSON 全体の書込みを await し、取消し時は既存の task scope が進行中の read/write と解放を終了する。

## 検査と計測

[model.test.wlt](model.test.wlt) の7検査で、レベル、空行、欠落フィールド、整数境界、独立したバケット、集計の桁あふれと更新の原子性、u64 最大値の十進数化を確認する。
Rust の検査はこれらを実行し、パーサ・集計・I/O の effect 境界も検査する。
Node は両バックエンドで、1 byteごとの UTF-8 入力、25,000行の集計、空入力、入出力エラー、待機中の取消しと非同期解放を検査する。
終了後の端点登録・ABI バッファは0、これらの負荷で線形メモリは64 KiB。Chromium でも両バックエンドを実行する。

`just profile-log-stats [出力先] [行数] [native反復数]` は次を分けて測定する。

- [benchmark.wlt](benchmark.wlt) は同じパーサと集計モジュールを、6種類の入力から呼ぶ。結果の checksum を検査し、Wasmtime の `--profile=guest,...,1ms` でサンプルを取得する。I/O adapter と実際の CLI を含まない、別の Wasm バイナリである。
- 実際の `main.wlt` を Core＋JSPI へコンパイルし、同じ行データを4 KiBごとに配送する。3回 warmup 後の7回で、main の完了までを測る。コンパイルとインスタンス化は含めず、出力 JSON 全体を独立した期待値と照合する。read カウンタは時間測定の後に別途取る。

2026-11-04、Apple M5 / macOS arm64、Node v24.21.0、Wasmtime 42.0.1。
最終版の30,000行・560,000 byte の Core＋JSPI 実行は中央値25.58 ms、JSPI read は30,002回、出力は1回だった。
終了時の ABI バッファは0、線形メモリは65,536 byte。native の30,000行×100回から312サンプルを取得した。
自己サンプルは stats.consume 約55%、parse.fields 約24%、parse.tab 約9%。割り込み点の偏りがあるため、正確な CPU 時間や JSPI の内訳とは扱わない。

### 条件を変えた交互比較

```sh
just profile-log-stats target/profile/log-stats-final
just compare-log-stats target/profile/log-stats-before-12346 target/profile/log-stats-final
```

保存した初版、1〜4の変更後、最終版を同じ Node プロセスで4回ずつ warmup し、順番を入れ替えて9回ずつ測定した。
コンパイル・インスタンス化を含めず、時間測定の後に独立した ABI 割当て probe を取る。
各実行で JSON 全体を独立した期待値と照合する。guest のハッシュと、probe が同じ allocator を使っていることを検査する。

| 条件 | 行数 | 初版 | 1〜4後 | 最終版 |
|---|---:|---:|---:|---:|
| ASCII 32 byte、不正行なし、4 KiB chunk | 16,000 | 13.05 ms | 12.95 ms | 12.80 ms |
| 日本語＋絵文字、不正行25%、4 KiB chunk | 16,000 | 16.20 ms | 16.03 ms | 16.23 ms |
| ASCII 4 KiB、不正行なし、4 KiB chunk | 1,000 | 5.27 ms | 5.32 ms | 5.31 ms |
| ASCII 64 byte、不正行50%、64 byte chunk | 16,000 | 22.87 ms | 22.65 ms | 22.27 ms |

この比較では概ね同等〜数%の改善で、大きな速度向上とはしない。ファイル I/O の throughput は測定していない。
1〜4後からの最適化は、入力行の enum の統合、集計バケットへの直接参照、更新前の値の再利用である。

16,000行の各条件では ABI 割当てが32,070→32,004回、JSPI read が16,035→16,002回、出力が34→1回。
1,000行の条件では ABI 割当てが2,070→2,004回になった。全条件・全版で線形メモリは64 KiB、終了時の生存ブロックは0。
これらの割当て数は線形 ABI バッファだけで、Wasm GC の割当て数ではない。

成果物は `target/profile/log-stats-before-12346/`、`log-stats-after-1234/`、`log-stats-final/`。
上の表の3版比較は `comparison-12346.json` に保存した。`comparison.json` は最後の比較コマンドの生の時間と probe、`report.json` に単独の native/JSPI 計測を保存する。
`guest.json`、実行用 Core＋JSPI、compute.wasm とソースも保存する。

## 残る不足

連結・整数変換・JSON string は標準ライブラリに移した。任意のキーによる group-by、汎用 JSON serializer、汎用 builder、WASI のファイル binding は残る。
Result/Stream のマクロは通常の型・effect・所有権検査を保つ。任意の例外を捕捉する finally や、所有 payload の配送へは拡張していない。
単相化ジェネリクスは今回の変更に含めていない。
