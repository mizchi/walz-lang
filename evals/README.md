# wlt の評価を繰り返す

在庫台帳を課題に、初見の読解・コードの可読性・コード変更の正しさを確認する。
使い回す課題、ルーブリック、外部契約、検証器をここへ分離した。過去の点数や回答を新しい評価役に渡さない。
suite は [stock-ledger](stock-ledger/suite.json) と [diagnostics](diagnostics/README.md)。以下は既定の在庫台帳評価の手順。エラーメッセージは `just eval-diagnostics-prepare ID` で準備し、同じ review/verify/score/compare を使う。
struct構築表記を比較する独立した読解課題は [struct-literals](struct-literals/README.md)。名前付け・波括弧・期待型リテラルを分けて評価する。
現在の在庫台帳suiteは、制御・所有性の読解とコード変更に加え、標準 WAT と `%` 拡張の区別、`wlt:core` の基本型、単相化、複数束縛の `%let`、解析の境界を質問する。必須回答は各 run の suite.answerKeys に固定する。5軸のルーブリックと各14件の実行検証は維持し、異なる suite 版の点差は比較しない。
以前の記録は [文法評価](../docs/grammar-eval/REPORT.md)、[可読性](../docs/readability/REPORT.md)、[文法改良後](../docs/readability/implemented/REPORT.md) に残す。

CLI・LSP・非同期・native WITと複雑なJSON処理を一通り試す評価は [開発体験の試用](usability/README.md)。実装者による非初見の試用と、ここで扱う初見の読解評価は区別する。

## 1. 新しい評価回を作る

`wlt-lang` で、Node 24以降と既存の開発依存を使用する。実行検証には Cargo、Lean（検証付きexport）、Wasm GC/JSPI が必要。

```sh
just eval-prepare trial-01
# 任意の場所にも作成できる。固定の階層深さや個人のパスに依存しない。
node evals/run.mjs prepare trial-02 --output /tmp/wlt-eval-trial-02
```

既存の評価回は上書きしない。ID に回数上限はなく、`evals/runs/` は Git 管理対象外。
現在の CHEATSHEET、main、行数付き版、契約、基準、課題に加えて依存する .wlt を support へコピーする。
suite v8 以降は生成済み標準API資料から必要な公開名の型・制約・effectを抜粋し、両役の `api.md` として固定する。
v9 は effect 契約の表記を `%effect` に統一する。既存の run の入力・回答・記録は変更しない。
v10 は標準型名を `$string` に揃える。課題と検査項目は v9 を維持し、新しい run へ入力を固定する。
v11 はWITに沿う標準型・関連関数と `%string` / `%tuple` / `%box` の語彙を使う。
v12 はJSON操作を `$json.*` にまとめる。課題の実行契約は維持し、過去の入力・結果は変更しない。
v16 は`wlt:core`からの`%use`と相対ファイルモジュールに統一する。`%namespace`は使わない。threads・固定atomicセルの検証モードを含む文法資料を配布する。旧suiteの配布物と改名前の入力アーカイブは変更しない。
main の include はコピー先の program.wlt から support を参照するため、後のライブラリ変更で評価コードが変わらない。
依存ファイルを増やしたら suite.json の support も更新する。

主な生成物:

```text
evals/runs/trial-01/
  dispatch.md           # 各初見エージェントへ渡す指示
  manifest.json         # suite、環境、実装fingerprint、役ごとの読取り範囲
  inputs.json           # 変更しない配布物・依存ソースのSHA-256
  reader/              # task、cheatsheet、api、契約、基準、main、counted
  writer/              # task、cheatsheet、api、契約、main、candidate.wlt、program.wlt
  support/             # 静的includeの依存ソース
  reviewer-task.md      # 親による回答監査の手順
```

## 2. 初見の読解役・変更役へ渡す

dispatch.md の2つの指示を、会話履歴を持たない別コンテキストへ渡す。CLI 自体はエージェントを生成しない。
読み書きの制限は評価指示であり、OS の sandbox ではない。

- **読解役**は reader 内の配布物だけを読み、reader.json へ回答と5軸の点数・根拠を書く。answers.syntax / bindings / standard は文法・初期化順・標準型の説明であり、自己採点だけで正解にしない。コンパイラは実行しない。
- **変更役**は writer 内の main をもとに candidate.wlt を編集し、成功値を配送行数へ変える。CLI の check/types/fmt は使用可。結果を writer/report.json へ記録する。

v8 の変更役は同じ candidate に `count64` と `format-count` も追加する。前者はfold状態・定数・演算・成功型のi64への変更、後者は標準APIを選んだ符号なし整数の解析とtext生成を検査する。

変更役へ counted の完成例、読解役の資料、依存実装、過去の結果を渡さない。通常の補助関数は許可するが、状態を pair に持つ条件は維持する。
candidate は最初は成功値0の main であり、未変更なら実行検証に失敗する。雛形をコピーしただけで合格にならない。

## 3. 回答を監査し、実行・集計する

親または別の監査役が reviewer-task.md を読み、reader.json の説明を配布コードへ照合する。
自己採点だけで合格にしない。監査した理由を具体的に記録する。

```sh
node evals/run.mjs review trial-01 --correct --reason '型・所有性・EOFとerrの説明を、配布したmainと契約の該当箇所へ照合した'
# 誤答なら --correct の代わりに --incorrect。
just eval-verify trial-01
just eval-score trial-01
```

run は ID またはパス。`/tmp/...`、`./somewhere/run` でも同じコマンドを使える。
review は回答のハッシュを固定する。回答を直したら再監査し、candidate を直したら再実行する。
配布資料や依存を変えたら、その評価回を継ぎ足さず新しい prepare から始める。

verify は reader の counted と writer の candidate をそれぞれ独立にビルドし、各14チェックを実行する。
型・Wasm 検証、pair 状態、初期化1回、配送行数と JSON、EOF/I/O err、read/write中の取消し、ハンドル表と ABI live=0 を確認する。
任意のGCヒープ全体のリーク不存在の証明ではない。Component/ブラウザまで変更した場合の回帰確認には既存の `just test-stock-ledger` / `just test-e2e` も使う。

同期 Wasm の無限ループも止めるため子プロセス全体を既定30秒で終了させる。環境に応じて `verify RUN --timeout-ms 120000` と変更できる。
ビルド生成物は work/、検証結果は verification.json。使用した実装の fingerprint と prepare 後の変更有無も記録する。
`wlt:core` の同梱ソースも実装の一部として fingerprint に含める。v7 の変更候補は main の初期化に複数束縛の `%let` があることも既存の W4 内で確認する。
v8 は候補に W5（i64のfold状態と実行）と W6（標準API、整数上限、不正入力、ABI live=0）を追加し、サンプル14件・候補16件を検査する。最終値だけi64へ変換する実装や固定のtextを返す実装を、ハーネスの負例で拒否する。
コンパイラ/ランタイム自体はコピーしないので、厳密な再現には記録した実装とツールチェーンを用意する。現在の実装で過去のソースを再検証することも可能。

summary.json の status:

| status | 条件 | score の終了コード |
|---|---|---:|
| pass | 全軸3以上、回答監査成功、変更役完了、両プログラムの全実行チェック成功 | 0 |
| fail | 必要な結果が揃っているが条件未達 | 1 |
| incomplete | 回答・変更報告・監査・実行が未完了、または更新前の結果 | 2 |
| invalid | JSONの形式や点数が不正、配布物のハッシュ不一致 | 1 |

実行成功を可読性へ加点せず、弱い1軸を高い総点で相殺しない。

## 4. 改善して次の回と比較する

```sh
just eval-prepare trial-02
# 新しい初見評価役で同じ手順を実施
just eval-compare trial-01 trial-02
```

比較は点数、合否、各軸の差とソースの計測値（トークン数・深さ・型の反復）を JSON で出す。suite / rubric のハッシュが異なる場合は点差を出さない。
基準を途中で緩めず、指摘→原因→コード/資料の修正→機械検証→初見評価を記録する。
同じモデルによる少数の主観点なので、点差を人間の作業時間や統計的な効果として扱わない。
必要な結果はディレクトリごと保存でき、相対 include を保ったまま別の場所で再検証できる。

保存した評価記録は [`%` 構文の v6](reports/20261106-prefix-v6/REPORT.md) と [標準型 import・複数束縛の v7](reports/20261106-standard-bindings-v7/REPORT.md) を参照。各記録には配布物、読解回答、変更候補、監査と実行結果を含める。
整形・命名・API資料の修正と、API選択・i64化の追加課題の結果は [v8の評価記録](reports/20261106-readability-v8/REPORT.md) に保存している。

## ハーネスのテスト

```sh
just test-evals
```

準備の上書き拒否、配布物改変、役の分離、点数形式、監査/候補更新後の失効、基準不一致、実際の正例/負例、同期 Wasm のタイムアウトを検査する。
旧 runtime/grammar-eval-prepare.mjs と verify.mjs は互換入口として残し、検証器本体は stock-ledger/verify.mjs に一本化した。旧 prepare の出力配置は新しい形式になる。
