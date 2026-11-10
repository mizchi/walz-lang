# WASI 0.3 ファイル値 adapter

[English](README.md) | 日本語

生成方法、明示的なファイル利用許可、所有権、制限は[adapter ガイド](../../docs/wasi-files.md)を参照してください。ゲストは完了を示す Future とともに `stream<u8>` として bytes を読み、descriptor はホスト内に留まります。

上流の参照先は WebAssembly/WASI `v0.3.0`、commit `3ee2a590c766594ae44a54730fc74fc27da5c609` に固定しています。ローカルの `filesystem.wit` は上流の resource world ではなく、Walz 向けに範囲を絞った値インターフェースを定義します。

`sum-batch(size)` は中断1回につき最大 `size` bytes を GC list へ読み込み、
`list.fold` で合計します。`main` は1 byteずつ読む stream の例を維持しています。

`generated/` を生成した後、CLI で2種類の転送サイズを比較できます。

```sh
WALZ_BENCH_INPUT=/absolute/path/input.bin walz bench examples/wasi-files/io_bench.walz \
  --target core-js --bindings examples/wasi-files/generated/bindings.json \
  --host examples/wasi-files/bench-host.mjs --samples 5 --iterations 1 --warmup-ms 0
```

各反復で、明示的に利用を許可したファイルを開いて閉じます。この計測はファイル I/O、
変換、JSPI、ゲストの処理を含む全体の時間を測ります。
