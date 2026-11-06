# std JSONエスケープの改善: 2026-11-06 v1

入力・Wasm・stdlibソース・計測driver・生サンプルをこのディレクトリに固定する。既存の記録は上書きしない。

## 条件

- Apple M5 / macOS arm64。Node v24.21.0 / V8 13.6.233.17-node.53。
- 同じ反復数でbefore/afterを31回交互に実行。ウォームアップ200ms、基準側の目標バッチ時間10ms。
- 反復はWasm内。入力文字列の構築、GC、結果の保持を含む。コンパイル・ウォームアップ・プロファイラは測定外。
- 比較は中央値のns/opと、対応するサンプル比の中央値。外れ値を削除しない。
- スカラー以外の結果の正しさはCore/Componentのoracle検査で別途確認。

## 結果

| 処理 | before ns/op | after ns/op | 対応比の中央値 | Wasm bytes before / after |
| --- | ---: | ---: | ---: | ---: |
| string.json ASCII 64 | 142.6 | 103.1 | 1.383x | 2356 / 2713 |
| string.json ASCII 1024 | 2005.3 | 1457.3 | 1.377x | 7640 / 7997 |
| string.json Unicode | 2490.9 | 1973.9 | 1.258x | 5035 / 5392 |
| string.json escaped | 541.2 | 515.3 | 1.047x | 2267 / 2624 |

入力構築、string/bytesのconcat、builder、JSON解析の7対照はWasmがバイト単位で同一。対応比は0.998〜1.006倍だった。
同一バイナリの時間差は測定のばらつきとして扱う。1台・この入力での測定であり、他のCPU・入力への性能保証ではない。

## 変更とプロファイル

`$string.json` は最初にエスケープを要するbyteを探す。なければwidth/putを呼ばず直接コピーする。
エスケープがあれば、見つかった位置から従来のサイズ計算を続け、先頭を再走査しない。
UTF-8検証、サイズのオーバーフロー検査、制御文字の `\u00xx` 表記、effect/所有権/ABIは維持した。
Wasmは357 bytes増加。ランタイムや言語プリミティブは追加していない。

1KiB ASCIIケースのV8 profileでは、変更前は810サンプル中657がstring.json、83がUTF-8検証だった。
変更後は803サンプル中577がstring.json、117がUTF-8検証。インライン化により補助関数は同じframeに含まれる場合がある。
Wasmtime 42.0.1の1ms guest profileでは、変更前は1734サンプル中widthが320、putが154。変更後の998サンプルでは両者のサンプルは0。
変更後はpacked bytesへの変換が約28%、UTF-8検証が約10%を占め、次に検討する箇所になった。
サンプル比は処理位置の判断に用い、プロファイル下の実行時間やエンジン間の性能比には用いない。

初期案でエスケープの多い入力に約6%の低下が見えたため、全体を別のサイズ計算に切り替える方式をやめ、従来の計算へ継続する構成にした。
最終の31回比較が上の結果。探索中の計測は `target/std-tune/` に別runとして残し、基準データは変更していない。

## 再測定

リポジトリのルートで、保存済みWasmを再測定できる。出力先は毎回変える。

```sh
just bench-compare benchmarks/stdlib-json/20261106-v1/before benchmarks/stdlib-json/20261106-v1/after target/std-json-recheck.json 31
just bench-profile benchmarks/stdlib-json/20261106-v1/after target/std-json-profile "string.json ASCII 1024" 200000
just test-std-tune
```

`comparison.json` の元パスは計測時の場所。移動後も各snapshot.jsonのSHA-256でファイルを検査できる。
CPU profileは `.cpuprofile`、Wasmtime guest profileは `.guest.json`。集計は各profileディレクトリのprofile.json。
使い方は [ベンチマーク](../../../docs/benchmarking.md) を参照。
