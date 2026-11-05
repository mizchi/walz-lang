# サンプルによる表現力と不足機能の確認

ソート、探索、ハッシュマップ、行列積、AST 評価、簡単なテキスト解析、状態機械まで実装できた。
閉じた variant と網羅的な match を追加し、AST はケースごとの型を持つ表現へ置き換えた。
Option/Result は組込みの値型として具体化し、effect 宣言の検査と公開関数からの WIT 生成も実装した。
所有値の move・終了規約、組・クロージャへの所有性の伝播、Lean で再検査する契約付き実行と整数の law も追加した。
配送済み/外部の Future と Stream を、所有値と suspends の操作契約へ接続した。Canonical ABI adapter を生成し、JSPI でホストのタイマーを待つところまで実装した。
標準 WAT を変更せず受け付ける経路を追加し、for と Wasm 例外の try/catch も標準命令へ展開して実行できる。

実用的なアプリケーションを書く際の不足は、ファイル・ネットワークなどの I/O binding、文字列操作の標準ライブラリ、公開する型の opaque 宣言、配列境界などの不変条件を証明する手段にある。
静的解析を重視するなら、構文を広げることに加え、副作用の対象や分岐後の事実を保持する必要がある。

## 確認した範囲

確認環境は Wasmtime 42.0.1、macOS/aarch64。通常の実行では GC・function references・exceptions を有効にしている。

| 検証 | 数 |
|---|---:|
| 型検査と Wasm 検証を通るモジュール | 32 |
| `main` の期待値と実行結果が一致するサンプル | 30 |
| 空入力、未発見、エラー値などの追加実行入力 | 31 |
| 意図した実行時失敗を確認するサンプル | 2 |
| 処理系が拒否する未対応機能の probe | 3 |
| 有効なサンプルに含まれる関数・クロージャの合計 | 88 |
| うち型付きコアで詳細に解析する関数・クロージャ | 84 |

ケースの正本は [examples/catalog.json](examples/catalog.json)。期待値を使うテストは、この一覧を読み込む。
関数の集計には law の検査用に生成する2関数も含む。
実行時失敗とコンパイル拒否を成功例に数えない。追加したソースが一覧から漏れた場合もテストを失敗させる。

```sh
just samples   # 実行可能な30本と期待値を表示
just probes    # 未対応機能の拒否理由を表示
just test      # 型検査、Wasm 検証、境界入力、実行時失敗も確認
```

## 実装できたパターン

| サンプル | 内容・結果 | 確認した性質 |
|---|---|---|
| [closure](examples/closure.wlt) | 生成したクロージャを返して呼ぶ → 42 | 捕捉値の寿命が呼び出し元より長い |
| [counter](examples/counter.wlt) | 捕捉した Cell を2回更新 → 42 | 参照の共有と明示的な変更 |
| [collections](examples/collections.wlt) | 配列の更新と総和 → 42 | 配列、添字、状態引数付きループ |
| [list](examples/list.wlt) | 再帰リストの総和 → 42 | 再帰型、nullable 参照 |
| [factorial](examples/factorial.wlt) | 10! → 3628800 | 0!、5! も確認。ループはスタックを増やさない |
| [mutual-recursion](examples/mutual-recursion.wlt) | 偶奇判定 → 1 | 前方参照、相互再帰、0 と奇数も確認 |
| [ast-interpreter](examples/ast-interpreter.wlt) | `(20 * 2) + 2` の AST 評価 → 42 | 再帰 enum、non-null な子、網羅的な分岐 |
| [variants](examples/variants.wlt) | Option の some → 42 | payload のない none → 0 も確認 |
| [graph-cycle](examples/graph-cycle.wlt) | 2ノードの循環を構築・走査 → 42 | 循環参照、参照同一性、参照型のループ状態 |
| [sorting](examples/sorting.wlt) | 挿入ソート → 12345 | 空配列、負数、重複要素も確認 |
| [binary-search](examples/binary-search.wlt) | 7 の位置 → 3 | 未発見 -1、先頭 0、末尾 4、ループの途中終了 |
| [matrix](examples/matrix.wlt) | 2×2 行列積の総和 → 134 | 3重ループ。結果の全要素 19/22/43/50 も確認 |
| [higher-order](examples/higher-order.wlt) | map と fold → 42 | 具体的なクロージャ型を引数で受け取る |
| [closure-compose](examples/closure-compose.wlt) | 2つのクロージャを合成 → 42 | 環境に別のクロージャを捕捉する |
| [result](examples/result.wlt) | 検査付き除算 → 42 | ゼロ除算と MIN/-1 を値として返す |
| [value-types](examples/value-types.wlt) | Option<Result<i64,i32>> の取り出し → 42 | 入れ子の値型、pure 契約、WIT への変換 |
| [ownership](examples/ownership.wlt) | 所有値の組とクロージャ → 42 | 明示 move、once-call、明示 close、Lean での再検査 |
| [verified-laws](examples/verified-laws.wlt) | law の証明後に加算 → 42 | 全 i32 入力の加算の性質、変更で law が破れると実行を拒否 |
| [ready-delivery](examples/ready-delivery.wlt) | Stream の総和と completion の確認 → 42 | 端点の move と close、終端と Result の区別、suspends、Lean の再検査 |
| [state-machine](examples/state-machine.wlt) | 状態2、イベント4回 → 204 | 可変 struct、明示的な遷移 |
| [text-parser](examples/text-parser.wlt) | ASCII の `12+30` → 42 | 空入力、不正文字、右辺欠落、末尾の余分な文字、0+0 |
| [hash-map](examples/hash-map.wlt) | 衝突・更新後の値の合計 → 42 | キー1/9の衝突、既存キーの更新、未発見 |
| [numeric](examples/numeric.wlt) | Newton 反復で √1764 → 42 | f64、0 と √81。標準 sqrt の全入力に対応する実装ではない |
| [host-import](examples/host-import.wlt) | コンパイル可、実行は未解決 import で失敗 | ホストとの接続が必要な境界 |
| [tagged-invalid](examples/tagged-invalid.wlt) | コンパイル可、実行は null trap | 手動のタグとフィールドの整合性は保証されない |
| [numeric-conversion](examples/numeric-conversion.wlt) | 標準 WAT の整数→浮動小数点変換 → 42 | 元の WAT を維持して受理 |
| [math-intrinsic](examples/math-intrinsic.wlt) | 標準 WAT の sqrt → 42 | フロントエンドの命令表に限定しない |
| [tail-call](examples/tail-call.wlt) | return_call で100000回反復 → 42 | 標準 Wasm の末尾呼び出しを受理 |
| [for-loop](examples/for-loop.wlt) | for の総和 + 36 → 42 | 明示的な状態と while/step、loop/br への展開 |
| [exceptions](examples/exceptions.wlt) | 除算 → 42、ゼロ除算の明示 throw → -1 | tag/throw/try_table、通常値と handler の結果 |

各例は小さな実装であり、汎用ライブラリとしての完全性を示すものではない。
例えば行列関数は入力配列のサイズを呼び出し側が満たす前提、ハッシュマップは8バケット固定、テキスト解析は `DIGIT+ "+" DIGIT+` に限定している。
数値のオーバーフローは現在の言語仕様どおり折り返す。文字列の Unicode 処理や、任意の f64 に対する数学関数の精度は今回の例では扱わない。

## 実際に行き当たった不足

| 不足 | 根拠 | 現状の対処 | 追加するなら主にどの層か |
|---|---|---|---|
| 共通の callable 型・型による汎用化 | [heterogeneous-closures](examples/probes/heterogeneous-closures.wlt) は同じシグネチャでも型不一致 | callback 型ごとの関数、または分岐する1つのラッパー型を書く | フロントエンドで単相化。動的ディスパッチは別途検討 |
| 外側ループへの直接の再反復 | [outer-recur](examples/probes/outer-recur.wlt) を拒否 | 内側の結果を外側へ返す、関数を分ける | 制御構文の糖衣。現時点の優先度は低い |
| 読み取り専用ビュー・freeze | [readonly-array-view](examples/probes/readonly-array-view.wlt) は型不一致 | 可変配列を返すか、不変リストなどを使う | まず opaque なコンテナ API。必要なら参照の権限・所有権 |

数値変換・sqrt・末尾呼び出しは標準 WAT 経路で受理できるようになり、probe から実行サンプルへ移した。
現在の拡張構文との混在は型付きコアの対応命令に限るため、これらを使う関数と拡張を同じモジュール内で自由に組み合わせることは次の課題である。
`include` は将来の表面構文の案を使った probe であり、標準 WAT の命令を意味しない。

### 末尾再帰の空間使用

通常の相互再帰 `even(100000)` は、Wasm スタックを32 KiBに制限すると `call stack exhausted` になった。
標準 WAT の `return_call` を使うサンプルは、同じ制限で100000回反復して42を返した。
loop/recur は定数スタックで実装できているが、一般の関数呼び出しにはまだその保証がない。

```sh
just build examples/mutual-recursion.wlt target/mutual-recursion.wasm
wasmtime run -W max-wasm-stack=32768 --invoke even target/mutual-recursion.wasm 100000
wasmtime run -W tail-call=y,max-wasm-stack=32768 --invoke main examples/tail-call.wlt
```

### 手動のタグはデータ不変条件を保証しない

`tagged-invalid.wlt` は「加算ノード」を表すタグ1と null の子を同時に持つ struct を作る。
すべてのフィールド型は正しいのでコンパイルできるが、子を使う位置で trap する。
GC によるメモリ安全性と、アプリケーションのデータ不変条件を静的に保証することは別の課題である。
この例は通常の struct を使う比較例として残す。variant を使う AST では、加算ノードの子は non-null 参照なので同じ不正状態を構築すると型エラーになる。
case の不足・重複、payload の型・個数の誤りも拒否する。
`tests/variants.rs` は再帰 enum、同じ payload 型を持つ別ケース、評価順序、字句スコープ、明示 capture、末尾 recur を実行して確認する。
外部 Wasm が基底型や不正なタグを渡す場合には実行時検査が残るため、match の `may_trap` は保守的に true とする。

### 不変の配列をどう作るか

`higher-order.wlt` の map は、動的な長さの出力を作るため、可変配列を割り当てて順番に埋める。
完成した配列を「以後は読み取り専用」として公開する仕組みは現在ない。別の不変配列型を宣言しても、参照をその型へ返すだけでは変換できない。
GC は寿命を管理するが、別名参照を通じた変更の禁止までは保証しない。
仕様を小さく保つなら、まずモジュールの可視性と opaque なコンテナ API で可変の内部表現を隠す方法を検討したい。

## 静的解析の到達点と不足

現在は型付きコアの80個の関数・クロージャについて呼び出し先と副作用を計算できる。
低水準の解析で可能性が立った関数の数は、可変メモリ読み取り29、書き込み24、割当て50、trap 63、throw 4、中断4、非停止49、ホスト作用2。
別途、標準 WAT の3モジュールに4関数があり、これらの詳細な解析結果は opaque として全作用を保守的に true にする。
同名のヘルパーもモジュールが異なれば別計上し、複数の作用を持つ関数はそれぞれに数えている。

| 実例 | 現在の解析 | 欲しい追加情報 |
|---|---|---|
| factorial の減少するカウンタ | `may_diverge=true` | 状態引数の範囲と減少量。終了を証明する ranking 情報 |
| ソートと二分探索の添字 | `may_trap=true` を保ち、facts に i32 区間と局所的な境界条件を追加 | 明示的なループ不変条件と関数間の範囲契約 |
| map が出力配列を埋める | memory.writes は allocation。入力と出力の長さの対応から get/set の境界を確認 | 対象別の effect 上限と、その情報を使う最適化 |
| sort が入力配列を書き換える | memory.writes は parameter。呼び出しと捕捉の代入も追跡 | 非別名性の証明。異なる引数が別物とは仮定しない |
| null 検査後の参照 | facts に正常な経路での non-null と null_check_proven を保持 | 検査の除去と意味保存の検証 |
| ホストの print | 全副作用が true に伝播する | ホスト実装と対応した、処理系側の信頼できる契約 |

クロージャの捕捉が明示的で、呼び出し先が閉じていることは解析の土台として機能している。
`closure-compose.wlt` の `$Compose` は `$Scale` と `$Add` への呼び出しを含め、全副作用が false になる。
一方、Boolean の作用だけでは「純粋な変換のための内部更新」と「外部状態の変更」を十分に分けられない。
追加した memory は、その対象を引数・捕捉・割当て・不明に分ける。参照の格納と返却、closure/値コンテナ、循環、loop/recur と関数間の伝播から escape を求める。
再帰のパスは深さ3で不明へ広げ、更新は古い候補を残す。型の上限へ広げたループ状態へ、初期値の範囲をそのまま流用しない。
これらは既存の作用契約や Wasm 出力を変更するものではない。新しい計画と完了範囲は [TODO.md](TODO.md) の6.1–6.6に残した。

別途、ソース上の effect 契約を実際の関数本体から検査する。呼び出し先の作用を固定点まで伝播し、try が処理した tag と handler から外へ出る tag を分ける。
Option/Result の格納用 GC オブジェクトはソースで同一性を観測できないため、意味上の allocates を持たない。payload の評価で生じる作用は引き継ぐ。
低水準の割当て・trap の解析は残し、pure 宣言だけで命令の除去や移動が安全だとは判断しない。生の Wasm との境界では格納参照の escape も考慮する必要がある。
未知の import は保守的に全作用を持つ。利用者が任意に pure を宣言することはできない。

WIT 生成は公開する数値型・Option/Result の契約を検証できる段階にある。GC 参照と未処理例外は拒否する。
Future/Stream の型と、読み取り関数の async も WIT に出力できる。別の Component 出力経路では、Canonical ABI adapter と JSPI 用の実行形式を生成する。
Node 24.21.0 と Chromium 156 で、GC 参照を保持した中断・再開、Promise/AsyncIterable の配送、イベントループの進行、pending Future の close、失敗した呼び出しの確定を実行した。
17引数の間接渡し、入れ子の variant、unit/none/EOF の区別も確認し、間接引数・複合戻り値を4万回受け渡してメモリの再利用を確認した。これらは32モジュールのカタログと別の Component fixture で検査している。
JSPI タスクの取消しも追加した。開始前はゲストを実行せず、開始後は pending read を終了させて unwind し、未消費の端点・バッファ・ホストの非同期解放を終了してから通知する。取消し後の再利用、catch_all の通過、返却との競合、100回の取消しと遅い配送、解放失敗による再利用拒否を確認した。

ローカルの Future/Stream は配送済みデータを GC に保持する。Future の一度限りの受け取り、Stream の順序・終端後の close、unit・GC 参照の配送、対象式の一度だけの評価を実行で確認した。
Stream の終端と別の completion Future の失敗も区別する。Lean の ready-only モデルでは、next の反復が take/drop に一致すること、closed 端点の利用を拒否することを検証する。
Component の close は drop-readable を実行し、固定ホストのタイマーを解放する。task cancellation では進行中の read も終了させる。writer や言語側の producer task はなく、任意の外部 producer を強制停止する保証は持たない。payload も共有できるデータだけに制限し、所有 payload の破棄 protocol は今後の課題とする。

所有権の核は、共有 GC とは別の own 値、Option/Result/pair への伝播、所有 capture の once-call、分岐・反復・終了時の所有集合として実装した。
Rust の検査に加えて、抽出した契約を Lean で再検査する。検査の成功が宣言的な利用規則を満たすことと、二重消費を拒否することを Lean で証明している。
law は実際の型付き関数を固定幅の BitVec に変換し、全入力の条件を Lean で検査する。単なる有限個の入力テストではない。
加算を減算へ変更して law を壊す場合、また x=65537 でだけ壊れる条件でも実行を拒否する。符号・折り返し・シフトのマスクも実行と照合する。
現在の law は純粋で非再帰の整数式に限る。分岐のある所有権契約を扱えることと、任意のループ不変条件を証明できることは別である。
コンパイラ全体の意味保存や外部 I/O の正しさの証明ではない。信頼する実装の境界は [proof/README.md](proof/README.md) に残す。

## 次に足す順序の提案

1. **実行環境と標準ライブラリの境界を作る。** 数値・文字列の入出力を提供するホスト binding と標準ライブラリを整える。bytes/text の表現と ABI、静的モジュールと名前の可視性は実装済み。
   テキスト解析を整数コード列で書けることと、ファイルや端末から文字列を扱えることには距離がある。
   ホストの作用契約は利用者の任意の pure 宣言にせず、実装と対応する binding 定義で管理する。
2. **標準 WAT と型付きマクロの混在を広げる。** WAT 自体は受理できる。次は未解析の命令を持つ関数にも型付きマクロを展開できるようにし、Wasm validator と型情報を共有する。
   詳細が分からない部分に純粋性を仮定しない。通常の call からの自動末尾呼び出し変換も別途検討する。
3. **解析した事実を契約へ接続する。** variant と網羅的な match、non-null・i32 区間・配列境界の解析初版は実装済み。次はループ不変条件と証明義務を扱う。
   外部から来ない enum 値を証明できれば、match の防御的な trap も解析から除外できる。
4. **対象別の解析を最適化へ接続する。** 引数・捕捉・新規割当てごとの読み書きと escape は実装済み。既存の作用上限を守り、解析を使う変換の意味保存を検査する。
5. **再利用は単相化から始める。** 要素型とクロージャ型を引数にするコンパイル時の汎用化で、map/fold の複製を減らす。
   具体化後の型と呼び出し先は閉じたままにする。実行時に異なるクロージャ型を混在させる場合は、有限の variant/wrapper を先に検討する。

外側への recur、finally、動的な callable の糖衣、共有 GC データのマルチスレッド利用の糖衣は、今回の実装を成立させるためには必要なかった。
これらを使うアプリケーション、非同期 I/O、外部リソースの解放・キャンセルは別のサンプルで検証する必要がある。
今回の確認は表現力と小さな実行例であり、性能・大規模コードの保守性・解析の完全性を評価したものではない。


bytes/text は不変な共有値として追加し、WIT の list<u8>/string へ変換した。
Node と Chromium で UTF-8、NUL、空データ、ネスト、Future/Stream、間接バッファの解放を確認する。
Wasm の UTF-8 検証は256通りの1バイト・65536通りの2バイト、境界例と乱数列を独立の TextDecoder と照合する。意味保存の形式証明ではない。
namespace/public/use と相対 include は静的に展開し、通常の型・effect・所有権検査に渡す。
名前の非公開性、別名、前方参照、ファイル循環、元ファイルの診断位置を検査する。旧 modules probe は実行サンプルへ移した。
汎用 list、文字列操作の標準ライブラリ、公開 type の opaque 宣言とフィールド単位の可視性は残っている。

## ストリーミングアプリケーションの検査

[ログ集計 CLI](examples/apps/log-stats/README.md) は5つの静的モジュールで、実際の TSV 入力から JSON の集計レポートまでを処理する。
パーサと数値変換はホストアクセスを持たず、可変状態を扱う集計と I/O の effect を分けて検査できた。
Core＋JSPI と Component の両方で25,000行、入出力エラー、取消しと解放を検査し、Chromium でも実行した。
その後、bytes.from-array と文字列ライブラリ、Result/Stream の限定マクロを追加し、通常の検査へ展開する形にした。
集計状態を opaque 型で隠し、レベルを enum にした上で、行 enum へ統合して中間オブジェクトを減らした。
JSON は1行で出力でき、コメントを保持する formatter も利用できる。比較条件と数値はアプリの README に記録する。
任意の group-by や汎用 serializer の実用性は、固定スキーマのこのアプリだけでは評価していない。

## 改善後の別アプリケーション

[在庫台帳](examples/apps/stock-ledger/README.md) を追加し、可変長の Unicode キー、GC のハッシュ表、失敗時に状態を保つ更新、JSON Lines を実装した。
言語・コンパイラ・ランタイムへの追加変更は必要なかった。手書きの制御フローと新しいマクロで同じ処理を記述し、Core / Component の両方で Map / BigInt の独立実装と照合した。
main は70→31行、S式は90→31に減り、手書きの close 3箇所をマクロへ移せた。外部からの状態変更と effect の過小申告は拒否された。
実行時間は再計測でほぼ同じだった。ただし展開後の一時 Result は増え、計算量や割当ての削減まで実現したわけではない。
文字列解析・具体的なコレクションのライブラリと、一時値の簡約が次の改善候補として残る。
