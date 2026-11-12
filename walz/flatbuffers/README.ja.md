# WalzでFlatBuffersを読み書きする

[English](README.md) | 日本語

公開方針と今後の作業は [TODO](TODO.md) にまとめています。

Wasm GCのarray、bytes、view、例外機構を使ったencoder／decoderのライブラリ。
型検査できる packed 配列と一括コピー命令を使い、ホストimportは不要。公式JavaScriptの
`flatbuffers`パッケージは相互運用テスト専用の開発依存になっている。

## 実行

リポジトリ直下で開発依存を`pnpm install`してから実行する。

```sh
just test-flatbuffers
walz run examples/flatbuffers/main.walz
walz bench examples/flatbuffers --list
walz bench examples/flatbuffers
walz build examples/flatbuffers/main.walz --target core-js -o target/flatbuffers
```

デモは`42`を返す。テストはネイティブWasmとCore＋JSPI／Componentの両方を使い、
Walz→公式JavaScript、公式JavaScript→Walzの変換を検証する。
benchはrecord構築、encode、検証、decodeした値の構築を含み、計測ループ内にホスト呼び出しはない。

## 性能比較

```sh
just flatbuffers-capture target/flatbuffers-bench/capture
just bench-flatbuffers target/flatbuffers-bench/capture target/flatbuffers-bench/run-1 9
```

保存先はそれぞれ新しいディレクトリを指定する。captureは入力・ソース・生成Wasm/glue・
公式JS実装とSHA-256を固定する。runはハッシュを検証して保存済みの生成物を測り、
ビルド時間は含めない。変更後は別の保存先にcaptureする。

v1の入力は小さいメッセージ、Unicodeと4 KiBのbytes、64 KiBのbytes、16,384個のi32を使う。
公式JS、WalzのGC値をWasm内に保持した場合、Core／ComponentのJS APIでencodeと全フィールドのdecodeを比較する。
JSのencodeはbuilderの新規作成・再利用の両方を測り、どちらも出力を独立したbytesとして返す。
residentは値のABI変換を省き、各操作のJS/Wasm呼び出しは含む。Core／Componentは入出力のABI転送を含む。
Walzのdecodeは境界・alignment・UTF-8を検証する一方、JS adapterは有効な入力のフィールドを信頼する。
zero-copyのアクセサだけを使うdecodeの計測ではない。

現行のv2 runnerは各variantを4,096回以上・250 ms以上warmupし、バッチ間でTaskのPromise処理を時間外に進める。
calibrationの後、計測順をずらして生の全sampleを保存し、全フィールドの一致とABI解放を時間外に検査する。
各sampleの直前に明示的GCを行い、時間内に起きた自動GCは計測に含む。
元の測定は[v1の比較記録](../../benchmarks/flatbuffers/20261111-v1/README.md)に保存している。
採用した最適化、paired-v3での比較、バンドルサイズ、SIMDの試行は[プロファイルと改善記録](../../benchmarks/flatbuffers/20261111-tuning-v1/README.md)を参照。

```sh
just profile-flatbuffers target/flatbuffers-bench/capture target/flatbuffers-profile
just size-flatbuffers target/flatbuffers-bench/capture target/flatbuffers-size /path/to/binaryen-133/bin/wasm-opt
just optimize-flatbuffers target/flatbuffers-bench/capture target/flatbuffers-O3 /path/to/binaryen-133/bin/wasm-opt
just compare-flatbuffers target/flatbuffers-bench/capture target/flatbuffers-O3 target/flatbuffers-comparison
```

プロファイルと速度比較は別々に行う。通常ビルドはBinaryenを呼び出さず、O3／Ozは任意の後処理になる。
後処理したcaptureは`WALZ_FLATBUFFERS_CAPTURE=target/flatbuffers-O3 node --expose-gc --test runtime/flatbuffers.test.mjs`でも検査できる。

## ファイル構成とAPI

| ファイル | 役割 |
| --- | --- |
| [reader.walz](reader.walz) | table、scalar、offset、string、vectorの検証付き読み取り |
| [builder.walz](builder.walz) | 末尾から組み立てる可変長builder、alignment、vtable |
| [サンプルのスキーマ](../../examples/flatbuffers/message.fbs) | Message、Child、識別子`WLZF` |
| [サンプルのadapter](../../examples/flatbuffers/message.walz) | スキーマに対応する型とencode／decode |
| [サンプルのentry point](../../examples/flatbuffers/main.walz) | 実行例とJavaScript向けexport |
| [サンプルのbench](../../examples/flatbuffers/codec_bench.walz) | Wasm内部のroundtripベンチマーク |

汎用モジュールは`walz/flatbuffers/`に置き、通常の相対パスの`@use`で使う。
`examples/flatbuffers/`のadapterからの参照例:

```text
(@use "../../walz/flatbuffers/builder.walz"
  (type $fb.builder) (func $fb.builder.new) (func $fb.builder.start)
  (func $fb.builder.bits-field) (func $fb.builder.end) (func $fb.builder.finish))
(@use "../../walz/flatbuffers/reader.walz"
  (type $fb.table) (tag $fb.invalid) (func $fb.root) (func $fb.bits))
```

スキーマ固有の型とadapter、main、benchは[サンプル](../../examples/flatbuffers/README.ja.md)に置く。
`$message.encode`は`(ref $Message)`から`$bytes`を返し、`$message.decode`は
`$bytes`から`(@inst $result (ref $Message) i32)`を返す。
Messageはi64のID、名前、i32のvector、bytes、bool、f64、OptionのChildを持つ。
省略された文字列・vectorは空、scalarはスキーマの既定値、Childはnoneになる。
`enabled`の既定値はtrue。recordは`@abi.type`でJS object／WIT recordに対応する。

JavaScript側の使い方は[英語版の例](README.md#layout)を参照。
IDはBigInt、valuesはInt32Array、payloadはUint8Array、Childの省略はundefinedを使う。
decodeは`{ tag: "ok", val: value }`または`{ tag: "err", val: code }`を返す。
同期処理なのでイベントループのホスト実装は不要。

## 検証とメモリ

バイナリ形式は[公式仕様](https://flatbuffers.dev/internals/)に従う。
tableの後ろに共有vtableがある形式も読める。有効な配置は一意ではないため、
相互運用テストではバイト列の一致ではなく値の一致を検査する。

offsetとlengthはi64に広げて計算し、範囲、alignment、table／vtableのサイズ、
既知フィールドの幅、vector長、UTF-8、文字列終端をアクセス・長さに応じた確保より先に検証する。
readerの`$fb.invalid`例外をMessage adapterでResultに変換する。

| エラーコード | 意味 |
| --- | --- |
| 1 | 範囲外のoffset、length、index |
| 2 | 不正な配置・alignment |
| 3 | 不正な識別子 |
| 4 | 不正なUTF-8 |

汎用readerのscalar幅は1／2／4／8。`$fb.bits`と`$fb.vector.get`はi64にbit列を返し、
符号拡張とfloatへの変換は明示的なWasm命令を使う。
`$fb.vector.bytes`は元の不変bytesを参照するzero-copyのbytes-viewを返す。
`$fb.i32s`と`$fb.string`、Messageの値構築とJS ABI境界ではコピーが発生する。
`$fb.root`はheader、getterはアクセスする既知フィールドを検証し、Message adapterはChildを含めた
全既知フィールドを読む。未知フィールドは互換性のため無視する。入力サイズの上限はアプリ側で設ける。

builderは信頼するプログラム内部の状態。子table／vector／文字列を先に作り、
`start`→fieldの追加→`end`→`finish`の順に使う。slotは0始まり。
offsetは同一builderのものを渡す。table構築中の入れ子構築、重複した書き込み、
不正なslot・幅、finish後の書き込みなどはtrapになる。
識別子は0または4バイト。finish後のbuilderは再利用できない。

## 対応範囲

scalar、tableと子table、UTF-8文字列、bytes／i32 vector、既定値の省略、
識別子に対応する。decoderは汎用scalar vectorも扱い、共有vtableを読める。
encoder側のvtable重複排除、`.fbs`からのコード生成、union、inline struct、
offsetのvector、size prefix、64-bit offset形式には未対応。

builderの上限はwire bytesとして64 MiB。作業用arrayは1バイトにつき packed i8 を使い、
拡張と文字列／bytes 書込みは一括コピーする。finish は一時配列を作らず有効範囲を不変 bytes にコピーする。

スカラーの読み書きは core の[固定幅アクセス](../../docs/byte-access.md)を使い、little-endian と全範囲の検査を明示する。
FlatBuffers 固有の整列・レイアウト・検証エラーの検査はライブラリ側で維持する。
文字列は指定範囲を UTF-8 検証してからコピーする。不正な text は payload のコピーを作らず、
従来の検証エラー4と BOM/NUL の規約を維持する。

空値、既定値、未知フィールド、整数限界、IEEE特殊値、Unicode、NUL、容量拡張、共有vtable、
各種不正入力、全prefix切断、固定seedの変異・ランダム入力をテストする。
成功／失敗を反復したあとABIバッファとホストhandleが残らないことも検査する。
GCリークや不正入力バグの不在を全面的に証明するものではない。

## 集計・変換・バッチ API

さらに小さい同期出力は [Direct GC / JS string ABI 実験](../../docs/js-native-experiment.md) で比較しています。
通常の Core／Component 出力と独立した実験用ビルドです。

[packed 配列・同期呼出し・バッチの測定記録](../../benchmarks/flatbuffers/20261111-three-improvements-v1/README.md)
に前後比較、最適化した JS との比較、サイズ、検証ログを保存しています。

[pipeline.walz](../../examples/flatbuffers/pipeline.walz) は `inspect`、
`inspect-batch`、`transform`、`transform-batch` を公開します。集計は Message/Child の
既知の全フィールドを検証し、values/payload を JS に展開せず、小さな Summary を返します。
`$fb.vector.sum-i32` は幅4の検証済みベクトルを signed i32 として i64 に合算し、
他の幅ではエラー2を返します。変換は decode → i32 の加算（wrap）→ encode を一度の呼出しで行います。
バッチは `list<bytes>` を受け取り、入力順に項目ごとの Result を返します。不正な項目があっても後続を処理し、空配列にも対応します。
新たな言語構文や永続ハンドルプールは追加しません。

```sh
walz build examples/flatbuffers/pipeline.walz --target core-js -o target/fb-pipeline
just bench-flatbuffers-pipeline target/flatbuffers-bench/capture target/fb-pipeline-run
```

事前に新しい capture を作成してください。`walz-flatbuffers-pipeline-v2` は同じ入力・結果で
従来の展開型、集計／一括変換、8件のバッチを比較し、結果と ABI 解放を計測外で検査します。
bytes の ABI コピーは残ります。JS の直接参照版は数値配列の view を使う信頼済み入力用です。
JS の変換は手書きループと通常／再利用ビルダーを比較し、Walz はレイアウトと UTF-8 も検証します。
ビルダーは packed i8 の作業領域と一括コピーを使い、finish は一時 i32 配列を作らず範囲を一度だけコピーします。
同期 API の通常呼出しは Task ハンドルと結果／完了 Promise を作らず、後処理の状態は維持します。
キュー実行する `start()` の契約は従来どおりです。
