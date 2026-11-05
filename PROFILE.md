# Guest profiler と実行メトリクス

`examples/io/text-filter.gwat` を対象に、Wasmtime の guest profiler で処理の偏りを調べ、実際の Core＋JSPI 出力で変更前後を比較する。
言語仕様と ABI は変更せず、境界変換と UTF-8 検証をチューニングした。

## 再計測

Wasmtime、Node.js 24 以降、pnpm と通常のビルド環境を使う。

```sh
just profile before             # 変更前の成果物とメトリクスを保存
# コンパイラまたはランタイムを変更する
just profile after              # 変更後を別の名前で保存
just profile-compare before after
```

`profile` は指定した名前のディレクトリに上書きする。比較元を残すため、変更前後で別の名前を使う。
同じ生成物同士の比較と、取得後に実行用の生成物が書き換わった比較は拒否する。
サンプルが50未満の場合は失敗するので、`just profile after 80` のように native 側の反復回数を増やす。

成果物は `target/profile/<名前>/` に保存する。

| ファイル | 内容 |
|---|---|
| `guest.json` | Wasmtime の Firefox Profiler 形式のサンプル。ファイルから開いて調査できる |
| `report.json` | エンジン・環境・生成物のハッシュ、自己／包含サンプル、ABI 割当て、Node の測定値 |
| `comparison.json` | `profile-compare` による再測定。各9回の生の時間と中央値を含む |
| `core.wasm`, `memory.wasm`, JS 一式 | その時点の実行用成果物 |
| `env.wat`, `driver.wat` と対応する Wasm | native 計測用ホストと検証ドライバ |
| `source.gwat` | 計測したソース |

`cargo run --locked -- profile-text <source.gwat> <output-dir>` は native fixture の生成コマンド。
対象は text I/O の read-lines/write-line を持ち、`main() -> result<i32,i32>` で処理行数を返す行フィルタに限定する。
ready/prepend など fixture が実装しない操作を行うと trap する。汎用の WASI ホストではない。

## 測定方法

1. **Wasmtime の調査**：Core＋JSPI 向けに生成した同じ `core.wasm` に、即時配送する Wasm ホスト fixture と本番と同じアリーナを接続する。`--profile=guest,<path>,1ms` で、30,000行を40回実行する。8行ごとに空行を入れ、非空行は64 byteの ASCII とする。処理行数、各実行後の端点と ABI バッファの解放をドライバで検査する。
2. **Node/V8 の時間比較**：保存した変更前後の成果物を同一プロセスで実行する。実際の JSPI、text I/O capability、4 KiB ごとの入力配送と SHA-256 を取る出力先を使う。入力生成・Wasm コンパイル・インスタンス化を時間に含めず、`api.main()` の完了までを測る。各条件で4回ずつ全量を warmup した後、変更前／後の順番を交互にして9回測る。全出力のハッシュと行数を毎回検査する。
3. **ABI のカウンタ**：時間測定をすべて終えてから、別の計測用アリーナで割当て回数・要求 byte 数・同時に生存したブロック数を数える。計測用 import の変更が、後の時間測定の tiering に影響しないようにする。JSPI read と出力回数もこの段階で数える。

Wasmtime のサンプルは関数入口・ループの割り込み点に偏る。ループ構造も変更するため、サンプル比率から高速化率を計算しない。
native fixture は非同期待機を行わず、V8 の実装や JavaScript のホスト処理の CPU 時間を測らない。実際の JSPI 経路の経過時間は2で比較する。
このベンチマークはメモリ上の入出力であり、ファイルやネットワークの throughput の測定ではない。
カウンタは ABI 用の線形メモリ割当てで、Wasm GC ヒープの割当て数や保持量ではない。保持の検査は [MEMORY.md](MEMORY.md) を参照。

## 2026-11-04 の結果

Apple M5 / macOS arm64、Node v24.21.0、Wasmtime 42.0.1 (`6844a83b5`)。
保存した `before` / `after` を使った。guest の SHA-256 はそれぞれ
`db030d448575ec3942bc776de7da6f981ab0cdc207baf6699c79af70d79962ca` と
`56fd76ff02f9d23785f993bd05aedf385cecf331e1cc4567c899345d0ff04e42`。

最初の544サンプルでは自己サンプルの約32%がアリーナの `free`、17%が取消し checkpoint、17%が buffer lift、8%が UTF-8 検証だった。
この観測を基に、次の2点を変更した。

- host へ渡す text/bytes の引数を直接 ptr/len に変換し、8 byte の一時 descriptor の割当てと解放を除去した。payload のコピーと解放、取消し checkpoint は維持する。不変な配列の長さを利用する。
- UTF-8 validator は文字境界で先頭が ASCII、かつ8 byte以上残っている場合に ASCII をまとめて検査する。非 ASCII が現れた位置で既存の検証へ戻し、末尾の短い入力も既存の経路で扱う。過長符号化、surrogate、範囲外、途中で終わる列の検証は保持する。

120万入力行・105万出力行の native fixture での集計：

| ABI メトリクス | 変更前 | 変更後 |
|---|---:|---:|
| 割当て回数 | 5,400,080 | 4,350,080（19.4%減） |
| 要求 byte 数の合計 | 165,600,800 | 157,200,800（5.1%減） |
| 同時生存ブロックの最大 | 2 | 2 |
| 線形メモリ | 65,536 byte | 65,536 byte |
| 終了時の生存ブロック | 0 | 0 |

変更後の514サンプルでも `free` が約31%で最上位だった。割当ての総量は減ったが、保持する線形メモリの大きさは同じだった。

Node の最後の交互比較の中央値。行の長さは UTF-8 byte 数、8行ごとの空行は別に含む。

| 非空行の内容 | 入力行数 | 変更前 | 変更後 |
|---|---:|---:|---:|
| ASCII 64 byte | 12,000 | 39.98 ms | 39.60 ms |
| 日本語＋絵文字 52 byte | 12,000 | 41.09 ms | 40.79 ms |
| ASCII／日本語混在 76 byte | 12,000 | 41.27 ms | 41.51 ms |
| ASCII 4,096 byte | 1,500 | 16.19 ms | 14.95 ms |

短い行・日本語・混在は約1%以内の差で、この測定ではほぼ同等。長い ASCII は、この比較で経過時間が約7.7%減り、直前の比較でも約4.9%減った。単一マシンの測定であり、これを一般的な高速化率とはしない。
12,000行の各条件では JSPI read が前後とも22,501回、出力が10,500回だった。境界通過の回数は変えていない。

固定の24行×2回で割当てが200回以下になる回帰検査を追加した（変更前220回、変更後178回）。
UTF-8 は ASCII の境界位置をずらした正常／不正列を `TextDecoder` と照合し、既存の全2 byte列・疑似乱数の検査も維持する。
空文字・NUL・BOM・日本語の host 引数を両バックエンドで往復し、解放も検査する。
