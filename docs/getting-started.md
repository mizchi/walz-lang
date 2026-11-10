# WLTを使い始める

WLTは、WATのS式を基底に、Wasm GCのデータと型付きの高水準構文を追加した言語。
まず数値を返すプログラムを実行し、テスト・ベンチ・外部への公開へ進む。
機能を先に知りたい場合は[機能一覧](features.md)、構文を順に学ぶ場合は[言語ガイド](language-guide.md)を読む。

## インストールと実行環境

Rust 1.99以降のCargoからインストールする。crates.ioからの配布は行っていない。

```sh
cargo install --git https://github.com/mizchi/wlt-lang --locked --bin wlt
wlt --version
```

チェックアウトから使う場合は、リポジトリ直下で`cargo install --path . --locked --bin wlt`を実行する。
インストール後は`wlt`があるディレクトリをPATHに含める。通常の利用にjustやpnpmは必要ない。

| やりたいこと | 追加で必要なもの |
| --- | --- |
| 型検査、Core Wasmへのビルド | なし。コンパイラにコアライブラリを同梱 |
| `wlt run`・`wlt test`の既定の実行先 | Wasm GC対応のWasmtime |
| Core＋JSPIで実行・ベンチ計測 | Node.js 24以降。外部の非同期処理にはJSPI対応が必要 |
| law・`%race-free`・検証付き実行 | elan経由の`lean +v4.34.1` |
| ブラウザで実行 | Wasm GC対応のブラウザ。非同期処理にはJSPIも必要 |

環境を入れずに構文を試す場合は[公開Playground](https://mizchi.github.io/wlt-lang/)を使える。
実行環境の詳しい条件は[CLI](cli.md)と[ランタイム](runtime.md)を参照。

## 最初のプログラム

空の作業ディレクトリに、計算を置く`math.wlt`と実行入口の`main.wlt`を作る。

```wat
;; file: math.wlt
(module
  (%export (func $add))
  (func $add (param $a i64) (param $b i64) (result i64)
    (%effect pure)
    (+ $a $b)))
```

```wat
;; file: main.wlt
(module
  (%use "./math.wlt" (func $add))
  (func $main (export "main") (result i64) (%effect pure)
    (call $add 40 2)))
```

```sh
wlt check main.wlt
wlt run                  # 42
wlt build                # target/main.wasm
wlt wat main.wlt         # 生成したWATを表示
```

`func`・`param`・`result`・`call`はWATの構文。`$a`はローカル参照の省略形、`+`は型に応じたWasm命令へ展開する演算子。
`40`と`2`は引数型からi64と決まる。整数幅の既定値や暗黙の型変換はない。

`(%effect pure)`はこの関数のsemantic effectを空に制限する。
`(%export (func $add))`は他のWLTファイル向けの公開、`(export "main")`はWasmの公開関数名。
2つの境界は[静的モジュール](static-modules.md)で詳しく説明する。

## テストを追加する

`tests/main.test.wlt`を作る。テストはライブラリから関数を選んで使う。

```wat
;; file: tests/main.test.wlt
(module
  (%use "../math.wlt" (func $add))
  (%test "add returns 42"
    (= (call $add 40 2) 42)))
```

```sh
wlt test                 # tests/以下の*.test.wltを実行
wlt test --list          # 検査と一覧表示。実行はしない
```

`%test`の条件はi32で、0が失敗、非0が成功。[テストの規約](testing.md)。

## ベンチマークを書く

`answer_bench.wlt`を作る。ファイル名を`*_bench.wlt`にすると`wlt bench`が見つける。

```wat
;; file: answer_bench.wlt
(module
  (%use "./math.wlt" (func $add))
  (%bench "add"
    (param $i i32) (result i64)
    (call $add (i64.extend_i32_u $i) 2)))
```

```sh
wlt bench --list
wlt bench --samples 10
```

反復番号で入力を変え、結果を返す。この小さい例は計測の入口を示すためのもので、実用処理の速度比較には使わない。
時間に含める範囲と計測オプションは[ベンチマーク](benchmarking.md)を参照。

## 実行先を選ぶ

| 用途 | 入口 | 詳細 |
| --- | --- | --- |
| Core Wasmとして数値計算する | `wlt build`・`wlt run` | [CLI](cli.md) |
| string・ResultなどをJSへ返す、Promiseを待つ | `wlt run --target core-js`・`wlt build --target core-js` | [ランタイム](runtime.md) |
| Viteから関数をimportする | `import {add} from './math.wlt'` | [Vite ESM連携](vite.md) |
| TS7でWLT importを型検査する | Content Mapper対応ホスト＋`contentMappers` | [TS7 Content Mapper](content-mapper.md) |
| WIT interfaceとして組み込む | `wlt build --target component` | [WIT binding](wit-bindings.md)・[native worlds](native-worlds.md) |
| ファイルや標準入出力を使う | 明示したホストcapabilityを渡す | [CLI](cli.md)・[WASI値adapter](wasi-files.md) |

Core WasmのGC参照はJSのstringやobjectへ自動変換しない。JSへ値を渡す場合はCore＋JSPIやComponentのABI変換を使う。
`wlt:core`はポータブルな型とデータ操作を提供し、OSのファイル・ネットワーク機能はホストから受け取る。

次は[基本の書き方](language-guide.md)へ進む。実際の複数ファイルプロジェクトは[CLIの例](../examples/cli/README.md)、
大きい処理は[ログ集計](../examples/apps/log-stats/README.md)と[ワークフロープランナー](../examples/apps/workflow/README.md)を参照。
処理系を変更する場合の環境構築とjustタスクは[開発ガイド](development.md)に分けている。
