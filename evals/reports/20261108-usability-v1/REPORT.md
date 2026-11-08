# 開発体験の試用 — 2026-11-08 / v1

JSONの検査・分類集計・エラー処理を組み合わせたプログラムは、現在の仕様で実装できた。複雑な処理を止めたのは表現力の不足より、ビルド対象の扱い、ジェネリクスの診断、エラー編集中のLSP、説明の食い違いだった。

評価者はこれまでの実装を知っている。同じ評価者がコードを書き、実行し、所見を付けた非初見の試用であり、人間の初見の成功率・作業時間・第三者の評点ではない。コンパイラやランタイムは修正していない。

課題と基準は [contract](snapshot/contract.md) / [rubric](snapshot/rubric.md)、機械的な結果は [result.json](result.json)、コマンドは [commands.json](commands.json)、実行条件は [manifest.json](manifest.json)。資料の評価時点の内容は [references](references)、最終コードは [snapshot](snapshot) に保存した。

## 一通り動かした結果

| 操作 | 結果 |
| --- | --- |
| READMEの例をcheck/run | 42を返す |
| JSONの分類集計、Core＋JSPI / Component | 各7組、独立したBigInt/Map oracleと一致 |
| Unicode・不正入力・64bit上限下限・加算overflow | 正しい件数と合計。overflowの行は以前の合計を壊さない |
| 1,500行・29グループの集計 | 両backendで一致 |
| 返金を許可→負数を拒否する方針変更 | 16行のモジュール追加とimport一行の変更。7組で照合 |
| CLI test / bench、core / core-js | 4通りとも成功 |
| 子のtimeout / cancelと兄弟の継続 | 両backendで42を返す。取得されたtimerはすべてdisposeされる |
| LSPの未保存import・UTF-16・hover・定義移動・修正 | 正常時と修正後は成功 |
| WIT binding生成・native resourceのborrowとclose | 5回実行してdestructorが5回。future<u32>も42を返す |
| test/benchを同居させたアプリのJSビルド | 失敗。内部のbench export名をWIT名として拒否する |

最終の操作チェックは **14成功 / 1失敗**。LSPの機能不足や診断の分かりにくさは、操作が成功しても別に評価している。集計・asyncの各呼出し後にABI生存数と端点tableが0であることも確認した。任意のGCヒープのリーク不存在を証明した結果ではない。

## 書き方が処理を邪魔していないか

[parse.wlt](snapshot/expenses/parse.wlt) の55–74行では、JSONのparse→必須field→型検査→整数検査→Row構築を `%result.do` で順に書けた。失敗するたびに外側を深く包む必要がない。`%$Row::{...}` で、構築時にフィールドの位置を覚える負担も減っている。

[model.wlt](snapshot/expenses/model.wlt) の46–66行は、既存の合計を取り出し、加算を検査してから辞書を更新する。失敗時に以前の値を保つ順序がコードに現れ、所有権検査を避けるための数値ハンドルやunsafe操作は不要だった。複数束縛で辞書・キー・金額をまとめて取り出せる点も役立つ。

一方、データアクセスの `struct.get`、辞書呼出しの `(%inst $dict.get i64)`、Option/Resultの結果型を明示する箇所は残る。特に「値を取り出す→OptionをResultにする→別の型へ検査する」というJSON境界は、処理の本質より型と包装の記述が目立つ。今回は汎用の `$require<T>` を一つ用意して抑えた。標準APIとしてOptionからResultへの変換やエラー変換をそろえる余地がある。

補助指標は本体118行、parser75行、実行入口4行で計197行。最大S式深さは本体10、parser7、`%inst` は本体10回・parser9回、`struct.get` は本体8回。整形済みコードの数値で、行数や括弧の深さだけで良否を判定していない。これ以上大きな処理では、関数・モジュールの分割が前提になる。

[parse-nonnegative.wlt](snapshot/expenses/parse-nonnegative.wlt) は既存parserに負数の拒否だけを加えたもの。同じRow/Errorを再公開できるため、集計・描画・ABIの型を書き換えずに差し替えられた。この変更ではモジュールの仕組みが負担を増やさずに機能している。

## 優先して直したい点

### 1. benchを置くと通常のアプリビルドが失敗する

4行の [最小再現](attempts/bench-repro.wlt) で、Core Wasmはビルドできるが、Core＋JSPIとComponentは次で失敗する。

```text
invalid WIT export name `__wlt_bench_0`; use lowercase kebab-case
```

利用者はこの内部名を命名していない。診断に従って自分の関数をrenameしても解決しない。`wlt bench` 自体は成功するため、計測を追加した後で通常ビルドだけが壊れる。

今回の回避は、処理を静的moduleへ移し、test/benchを別ファイルへ置くことだった。実行入口は既存WATの `(export "main" (func $sample))` を使って4行で作れた。新しいexport構文は不要だった。根本の修正は、通常ビルドのABIに内部bench exportを混ぜないこと。証拠は [Core](attempts/bench-core.log) / [Core＋JSPI](attempts/bench-minimal.log) / [Component](attempts/bench-component.log)。

### 2. ジェネリクスの型エラーを元の型で表示する

辞書呼出しの型引数をi64からi32へ間違えたときの実際の診断は次だった。

```text
type mismatch: expected (ref $wlt-core-0-dict.dict.mono30), got (ref $wlt-core-0-dict.dict.mono20)
```

関連位置も `wlt:core` の1837行と `.mono23` を指し、利用者には「dict<i32>が必要だがdict<i64>を渡した」と読めない。内部IDを解析するため実装へ戻る負担が大きい。

未知のローカル `$prevous` には `did you mean $previous?` と束縛位置が出る。enum生成に必要なallocatesを忘れた場合も、必要なeffectと発生操作が明示され、こちらは直しやすかった。診断全般が弱いのではなく、単相化後の型表示が弱点である。[実診断](result.json) の最後のチェックに両例を保存した。

### 3. エラー編集中も使えるLSPにする

未保存の別moduleへの定義移動と、型エラーの位置・修正後の復旧は動いた。しかし呼出し引数を一つi64へ間違えただけで、正常な束縛へのhoverと、変更していない関数への定義移動も両方 `null` になった。今回のような長いmoduleでは、修正の手掛かりを最も必要とする時に情報が消える。

またcompletionは `-32601 method not found`。これは現時点の仕様どおりだが、長いcore API名と明示importを入力する作業では不足を感じる。優先順位は、ファイル全体が型検査に成功しなくても安全な範囲の名前索引・位置情報を残すこと、次に既存APIの補完。過去の型や位置を無検査で新しいコードへ流用することは勧めない。

正常時のhover/definitionはこの一回では約0.2ms。これは短い入力・ローカルstdioの参考値で、実エディターの操作感や大規模ファイルの性能を示さない。

### 4. 更新済み機能の説明を同期する

評価時の [scoped-tasks.md](references/docs/scoped-tasks.md) 24行には「子だけの個別取消しは扱わない」とあるが、後半では実装済みの `%task.cancel` を説明している。[resources.md](references/docs/resources.md) 30行はnative own/borrowを後続としているが、[native-worlds.md](references/docs/native-worlds.md) では実装済みとして説明しており、試用でも動いた。

CLIのbackendごとの役割も意識する必要がある。GC文字列を返すアプリを既定のcoreでrunすると `<anyref>`、core-jsではJSONエンコードされた文字列が出る。これは同じ表示ではなく、raw textのフィルターにするには明示的なI/O接続が必要。結果型と用途から実行コマンドへ案内する説明があると迷いにくい。

### 5. 文字列リテラルの適用範囲と診断をそろえる

名前付き構築では裸の文字列を使える一方、通常のcall引数では `%string.lit` が必要。裸の文字列を渡したときの診断は `expected folded expression` だけだった。また `%string.len` と推測して書いた場合は、ソースにない `text.len` が診断に出た。実際には `%bytes.len (%string.bytes value)` と書けばよい。

仕様を増やす場合は文字列の期待型が確定する位置に限定できる。先に、既存の書き方・リテラルを使える位置・診断の修正例を明確にするだけでも負担を減らせる。[初期試行](attempts) を保存してあり、後から成功したコードに置き換えていない。

## ルーブリックによる所見

| 軸 | 点 / 4 | 根拠 |
| --- | ---: | --- |
| 入口の発見 | 2 | README例は動く。backendの選択と新旧資料の食い違いに確認が必要 |
| 処理の表現 | 3 | Result連鎖・複数束縛・名前付き構築で処理順を保てる。JSON境界の包装は多い |
| 型と所有権 | 3 | Row/Errorを分けて集計でき、resourceは18行のsourceでusing/borrowを接続。型引数の反復は残る |
| 診断からの復旧 | 2 | 未知名・effectは良好。generic内部名、文字列とmatch構文の案内は弱い |
| 変更の局所性 | 3 | 負数方針は16行と既存import一行で変更。大規模な型変更までは試していない |
| ホスト接続 | 3 | 生成bindingとown/borrow、SDK子取消し、native futureを実行できた。backend固有の準備は必要 |

総合点で弱点を相殺しない。現時点では、さらに大きな機能を増やす前に上の1–4を直す効果が大きそうだ、という評価者の判断である。新しい一般的な型推論や構文が必要だとまでは、この試用から断定しない。

## 再実行と評価器の修正履歴

```sh
just eval-usability fresh-trial
just test-evals
```

最終の `usability-20261108-v1-r3` を保存した。評価器のテストは19件通過。元の凍結済み評価・計測スナップショットは変更していない。

- r1: 評価者が `wlt fmt` の空のstdoutを書き戻して試用コードを空にした。正常な試用ではなく、集計から除外。保存済みattemptから復元した。fmtはファイルを直接更新する。
- r2: Coreの取消しで「開始前に取り消された子はtimerを取得しない」ことを、disposeが2回あるはずという評価器が誤判定した。言語の不具合ではない。
- r3: 契約に沿って「取得されたtimerが全てdisposeされる」を検査し、Unicodeを含む行のUTF-16位置でもLSPを照合。14成功・1失敗。r1/r2との点差を言語の改善として扱っていない。

今回はバッチ入力のプログラムとstdioクライアントによる試用である。人間による初見評価、実エディターでの入力や補完、Playground UI、長時間の編集、thread/atomicsを含む開発、ストリーミングアプリの新規実装、cold installの所要時間は未評価。
