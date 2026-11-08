# WASI 0.3 のファイル値 adapter

参照する仕様は [WASI v0.3.0](https://github.com/WebAssembly/WASI/releases/tag/v0.3.0)、commit `3ee2a590c766594ae44a54730fc74fc27da5c609` に固定する。`proposals/filesystem/wit/types.wit` の `descriptor.open-at` / `read-via-stream` / `write-via-stream` をホスト側で呼ぶ。

WLT に渡す API は `wlt:filesystem/files@0.3.0`。これは WLT の値 adapter であり、`wasi:filesystem` の resource ABI を名乗らない。descriptor はホストが所有し、guest は選んだ名前、byte stream、完了 future、result のエラーだけを受け取る。

```sh
wlt bind examples/wasi-files/filesystem.wit examples/wasi-files/policy.json examples/wasi-files/generated
wlt run examples/wasi-files --target core-js \
  --bindings examples/wasi-files/generated/bindings.json --read-file input=./input.bin
wlt run examples/wasi-files --target core-js --invoke write \
  --bindings examples/wasi-files/generated/bindings.json --write-file output=./output.bin
```

例の `main` は全 byte の合計、`write` は `AB` を出力する。生成先が存在する場合、bind は上書きを拒否する。

`read-file(name)` は `tuple<stream<u8>, future<result<_, error-code>>>`。Stream を読み切ってから完了 Future を待つ。`write-file(name, stream<u8>)` は完了 Future を返す。読み取りは前取りせず、書き込みは backpressure に従う。early return / cancellation は reader と descriptor を閉じ、完了と取消しの通知は解放を待つ。

`runtime/wasi-filesystem.mjs` の `createWasiFileSystem` は、ホストが渡す `{directory: descriptor, path: 選んだ相対パス}` の map に接続する。`runtime/node-filesystem.mjs` は同じ protocol を Node の FileHandle で提供する。Node 版は明示された個々のファイルだけを扱い、guest のパスから任意のディレクトリを検索しない。

返す error-code は bad-descriptor / interrupted / invalid / io / no-entry / not-permitted / read-only に絞る。不明なホストエラーは io にする。未許可の名前はファイルを開かず not-permitted。

このWASIファイル値adapterはnativeのdescriptor resourceをguestへ公開しない。全error-code、preopenのguest公開、ネットワーク、WASI一式の元worldのnative接続は未対応。importしたresourceの型付きown/borrowを扱う汎用adapterは [native worlds](native-worlds.md) を参照。未知の host Promise を強制停止することはできず、取消し時も provider が解放を完了することが前提。

reader は `readMany(max)` にも対応し、Node 版は16 KiBの内部 buffer から上限以内の byte chunk をコピーする。
write も対応する reader から batch を受け、短い write でも残りを最後まで書く。
`examples/wasi-files` の sum-batch と io_bench が [明示的なバッチ転送](stream-batches.md) を使う。
