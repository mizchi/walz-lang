# WALZアプリケーション

[English](README.md) | 日本語

`walz new`で生成したアプリケーションです。WALZはWATのS式に、型付きの高水準構文とWasm GCのデータを追加する言語です。

```sh
walz check src/main.walz
walz run                    # 42
walz build                  # target/main.wasm
walz test                   # tests/**/*.test.walz
walz bench --list            # app_bench.walzを検査・一覧表示
walz bench --samples 10
```

`run`と既定のテストにはWasm GC対応のWasmtime、ベンチと`--target core-js`にはNode.js 24以降が必要です。
このテンプレートのビルドと型検査は、インストール済みの`walz`だけで動きます。

| ファイル | 用途 |
| --- | --- |
| `src/main.walz` | Wasmへ`main`を公開する実行入口 |
| `src/math.walz` | `@export`でAPIを明示したライブラリ |
| `tests/math.test.walz` | ライブラリの公開関数を選んで検査するテスト |
| `app_bench.walz` | 反復ごとに入力を変えるベンチ |
| [CHEATSHEET.md](CHEATSHEET.md) | 英語の文法リファレンス |
| [CHEATSHEET.ja.md](CHEATSHEET.ja.md) | 日本語の文法リファレンス |
| [.agents/skills/walz/SKILL.md](.agents/skills/walz/SKILL.md) | WALZを読み書きするためのskill |

このディレクトリでコマンドを実行します。`walz build`・`walz run`は`src/main.walz`を自動で選びます。
ライブラリの公開・利用には`@export`・`@use`、ホストやWasmとの境界にはWATの`import`・`export`を使います。

JSへ配布する場合は`walz build --target core-js -o dist/app`、Componentには`walz build --target component`を使います。
ホストimport・Vite・TS7 Content Mapper・ABIの対応範囲は[WALZの資料](https://github.com/mizchi/walz-lang/blob/main/docs/README.md)を参照してください。

このベンチはCLIの操作例です。非常に小さな計算はランナーの負担が支配的になり得るため、実用的な処理単位で計測してください。

Codexは`.agents/skills`のプロジェクト内skillを検出できます。他の対応エージェントでは`walz`フォルダーをそのskillディレクトリへ配置できます。
参照資料は同梱しており、ジェネレーターがグローバル設定を変更することはありません。
