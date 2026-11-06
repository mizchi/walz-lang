# JSONパーサーのチューニング

2026-11-06、Apple M5 / Node v24.21.0 / V8 13.6で、[初回の比較](json-parser-bench.md)を基準に変更前後のバイナリを同じプロセスで測定した。
言語仕様や公開APIは追加していない。数値の原文、重複キーの順序、UTF-8 byte単位のエラー位置、65,536 bytes・128段の制限を維持する。

## 採用した変更

- 空白・数字の走査では長さをループの外で取り、同じbyteを繰り返し読まない。
- 文字列の通常部分は8byteずつ走査する。ブロック全体の範囲を確認してから読み、最初の引用符・バックスラッシュ・制御文字で止まる。末尾は1byteずつ処理する。
- エスケープがない文字列はbuilderを作らず、一度のsliceで返す。builderは最初のエスケープが現れた場合だけ作る。
- bytesのsliceは、新しいGC配列への`array.copy`に展開する。範囲検査とUTF-8検査は維持する。

## 変更前後の結果

入力がGC上にある解析本体の中央値。単位はµs/文書。

| 入力 | 変更前 | 変更後 | 速度比 |
|---|---:|---:|---:|
| small-object | 0.37 | 0.25 | 1.49x |
| records-128 | 53.44 | 33.48 | 1.60x |
| records-512 | 220.39 | 139.80 | 1.58x |
| numbers-4096 | 138.20 | 132.17 | 1.05x |
| wide-object | 133.01 | 88.37 | 1.51x |
| ascii-string | 124.14 | 26.83 | 4.63x |
| unicode-escapes | 19.41 | 19.85 | 0.98x |
| nested-64 | 1.26 | 1.03 | 1.21x |
| whitespace | 54.74 | 26.92 | 2.03x |

40KBレコードの入出力ABI込みの解析＋再シリアライズは、Core＋JSPIが520.10→420.74µs、Componentが545.44→448.25µsだった。
解析本体の1.58倍に対して、API全体は約1.22–1.24倍の改善となる。数値配列とUnicode escapeには大きな変化がない。
resident Wasmは12,220→13,174 bytes。初回比較の時間とは別の実行であり、この表は同じプロセスで再測定した変更前を使う。

[9回比較の全標本・入力hash・サイズ](../benchmarks/json-parser/20261106-r7-tuning/report.json)、[全APIの表](../benchmarks/json-parser/20261106-r7-tuning/report.md)、[試行ごとのソースと結果](../benchmarks/json-parser/20261106-r7-tuning/)を保存した。

## 試して採用しなかった変更

構築専用の32要素chunkへ値を溜め、最後にimmutableなJSONノードを一度だけ作る方式も試した。
スカラー版との5回比較では、数値配列は約135→112µs、幅の広いobjectは約87→77µsに短縮した。
一方、40KBのレコード配列は約141→166µsになった。小さな配列やobjectの追加バッファが負担になるため、この方式は採用していない。
公開済みノードを破壊的に反転する方式も、問い合わせ関数へ`reads` effectを持ち込むので採用していない。`get` / `at`をpureな関数から呼べることを回帰検査する。

SIMDについては、GC bytesから16byteをレジスタへ詰め、`i8x16.eq`・`i8x16.lt_u`・`i8x16.bitmask`で分類する方式を試した。
スカラー版の48KB ASCII文字列は約30→24µsになったが、短いキーの多い40KBレコードは約140→167µsに遅くなった。
先頭8byteをスカラーで調べてからSIMDへ進む版でも、レコードは約141→154µsだった。最終版はSIMDを使わない。

[simdjsonの設計](https://github.com/simdjson/simdjson/blob/master/HACKING.md)では、最初の走査で構造位置を索引化し、その後に値を構築する。
今回の実験は文字列のブロック分類だけであり、この二段階パーサー全体の実装ではない。
現状のGC byte配列には線形メモリの`v128.load`を直接使えず、16回の読み出しとlane挿入の費用が残る。
今後SIMD化する場合は、入力の格納・転送方法と合わせて検討する。

## 測定方法

変更前の`20261106-r2`と変更後の`20261106-r7-unroll`を固定し、入力とベンチマーク用WLTのSHA-256が同じことを検査した。
入力の準備・コンパイル・instantiate・出力照合を測定から除外し、3バッチのwarmup後、順番を回転して9回測る。
明示的GCは各バッチの前だけに行い、バッチ内の自動GCは時間に含める。
実際のGC ASTを返すresident解析と、Core＋JSPI / Componentの解析状態・再シリアライズ・入力長の対照を分けて測る。
全バッチ後にASTの再シリアライズhash、正規化した文字列の完全一致、ABIバッファの未解放数0を検査する。

過去のcaptureと結果は書き換えない。試行ごとのバイナリと全ソースは`target/json-parser-bench/`の`20261106-r3-scalar`、`20261106-r4-chunks`、`20261106-r5-simd`、`20261106-r6-simd-prefix`、`20261106-r7-unroll`に保存した。

```sh
just test-json
just test-json-bench
just json-bench-capture target/json-parser-bench/new-tuning
just json-bench-compare target/json-parser-bench/20261106-r2 \
  target/json-parser-bench/new-tuning target/json-parser-bench/new-comparison 9
```

比較用の実装も結果と一緒に保存する。保存した`compare.mjs`を`node --expose-gc`で実行すれば、同じ比較を別の新しい出力ディレクトリへ再実行できる。

## 検査

`just check`、Rust 327テスト、GC有効のNode 83テスト、Chromium 48テストが通過した。
短い文字列から48KBまでの通常文字列、8/16byte境界の全位置、Unicodeとescape、切れた入力・制御文字のエラー位置、bytes sliceの全小区間と範囲外を検査する。
元のJSON値を保持したまま`prepend` / `set`した結果と両方を観測し、不変性と重複キーの順序も確認する。
