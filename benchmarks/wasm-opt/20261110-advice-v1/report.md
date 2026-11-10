# 性能警告と明示的な書き換えの確認

ループ内でbytesをsliceしてから先頭を読む例を、元データの先頭を直接読む例と比較した。
これはコンパイラによる自動変換ではなく、任意警告を受けてソースを明示的に書き換えた比較になる。
過去のmatch loweringの測定とは別の入力・記録である。

| 入力 | `W_OPT_LOOP_COPY` | raw全体 / code section | Binaryen `-O3` 全体 / code section | `-O3` 後の `array.copy` |
| --- | ---: | ---: | ---: | --- |
| [loop-copy.wlt](inputs/loop-copy.wlt) | 1 | 1109 / 681 B | 253 / 175 B | あり |
| [loop-indexed.wlt](inputs/loop-indexed.wlt) | 0 | 1048 / 649 B | 214 / 136 B | なし |

GCとreference-typesを有効にしたBinaryen 133の通常のopen-world `-O3` を使った。
closed-worldやtrapを無視するオプションは指定していない。速度はこのrunでは測定していない。
環境とツールは [toolchain.json](toolchain.json)、現在のソース・実行ファイルのハッシュは
[manifest.json](manifest.json)、全コマンドと出力は [commands.json](commands.json)、集計は [result.json](result.json) に保存した。
`*.o3.txt` はBinaryenのmetricsとIR印字で、WLTの入力WATとは区別する。

両方のraw／最適化後Wasmで `main=42` をWasmtimeとV8で確認した。
V8では長さ0／1／2／1024、反復0／1／2／17を使い、結果と空データの境界trapを照合した。
反復0回なら空データでも0を返し、反復1回なら境界trapになる。生成コードのハッシュも保存している。

性能警告は型付きIRのコピー操作・呼出し先・領域解析から出す助言で、Binaryen自身の失敗通知や
実行頻度の判定ではない。ホストescapeの別テストでは、最適化前後とも17回のホスト呼出しが
17個の異なるGC参照を受け取ることを確認した。同一性を必要とする境界を勝手に書き換えないための検査になる。

```sh
just check-performance examples/optimization/loop-copy.wlt
wlt check examples/optimization/loop-indexed.wlt --warn-performance --json
just test-performance-warnings
just test-wasm-opt /path/to/binaryen-133/bin/wasm-opt
```

診断の仕様・LSPの有効化・除外条件は [wasm-optの説明](../../../docs/wasm-opt.md#最適化しにくいソースへの任意警告)を参照。
保存した結果は上書きせず、再調査は新しいrunを作る。
