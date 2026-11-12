# FlatBuffersのMessageサンプル

[English](README.md) | 日本語

`walz/flatbuffers/`の[FlatBuffersライブラリ](../../walz/flatbuffers/README.ja.md)を使うサンプル。
スキーマ固有のコード、実行用main、benchを汎用reader／builderから分離している。

| ファイル | 役割 |
| --- | --- |
| [message.fbs](message.fbs) | Message、Child、識別子`WLZF`のスキーマ |
| [message.walz](message.walz) | スキーマに対応する型とencode／decode |
| [main.walz](main.walz) | `42`を返すデモとJavaScript向けexport |
| [codec_bench.walz](codec_bench.walz) | 値の構築を含むWasm内部のroundtripベンチマーク |
| [resident.walz](resident.walz) | 比較ハーネス用のGC参照の準備・読み出し。JSの値のABIは使わない |

リポジトリ直下で`pnpm install`してから実行する。

```sh
just test-flatbuffers
walz run examples/flatbuffers/main.walz
walz bench examples/flatbuffers --list
walz bench examples/flatbuffers
walz build examples/flatbuffers/main.walz --target core-js -o target/flatbuffers
```

同じディレクトリの別ファイルからadapterを使う例:

```text
(@use "./message.walz"
  (type $Message) (func $message.encode) (func $message.decode))
```

`$message.encode`は`(ref $Message)`から`$bytes`を返す。
`$message.decode`は`$bytes`から`(@inst $result (ref $Message) i32)`を返す。
省略された文字列・vectorは空、scalarはスキーマの既定値、Childはnoneになる。

JavaScript側の使用例:

```js
const { instantiate } = await import("./target/flatbuffers/index.js");
const { api } = await instantiate();
const value = {
  id: 42n, name: "日本語🦀",
  values: Int32Array.of(20, 22), payload: Uint8Array.of(0, 128, 255),
  enabled: true, weight: 1.25, child: { count: 42, label: "child" },
};
const bytes = api.encode(value);
const decoded = api.decode(bytes); // { tag: "ok", val: value } or { tag: "err", val: code }
```

Childを省略する場合はundefinedを使う。同期処理でホストimportは不要。
検証エラー、viewとコピー、builderの規約、対応形式は[ライブラリの説明](../../walz/flatbuffers/README.ja.md#検証とメモリ)を参照。

`just test-flatbuffers`はネイティブWasmとCore＋JSPI／Componentで公式JavaScript実装との相互運用を検査し、
不正入力への対応とABIバッファ・handleの解放も確認する。

公式JSとのencode／decode別の比較と入力の保存は、[ライブラリの性能比較タスク](../../walz/flatbuffers/README.ja.md#性能比較)を使う。
