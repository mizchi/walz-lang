# 現行WLTの性能証拠と置換可能性

## 既存計測は何を示しているか

### Takeaway
実測で強いのは、GC上のpack済みデータを保持したSIMDカーネルである。公開の値ABIを毎回通す単発APIは、転送改善後もJSより遅い。汎用JSON parserや行ごとの非同期処理についてJSライブラリ全体を置換できる証拠はまだない。今回新しいベンチマークは実行していない。

### Cited Findings
- 2026-11-06、Apple M5 arm64、Node v24.21.0 / V8 13.6、9標本の中央値。16,384要素residentでは、i32 sum JS 8.719µs / WLT SIMD 1.902µs（4.58倍）、f32 dot 10.349 / 2.537（4.08倍）、bitmap intersection count 16.000 / 2.878（5.56倍）、bytes miss検索 3.786 / 0.534（7.09倍）。16要素や先頭一致にはJSが速いケースがある。 — [計測条件と表](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/simd-kernels.md:55)、[固定した全標本](/Users/mz/ghq/github.com/mizchi/wlt-lang/benchmarks/simd-kernels/20261106-r1/report.json)
- residentは入力の構築・setter転送・packを時間外にしている。比較対象jsimdはcommit `2cf51e8885314764dcb44b6469899d95f204b148` の保存済み生成バイナリを使う生カーネルで、公開JS wrapperやallocatorを含まない。bytes scalarの格納はi32配列、SIMDは128bitに圧縮した表現であり、メモリ密度も異なる。 — [比較の範囲](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/simd-kernels.md:43)、[capture metadata](/Users/mz/ghq/github.com/mizchi/wlt-lang/benchmarks/simd-kernels/20261106-r1/capture.json)、[resident初期化](/Users/mz/ghq/github.com/mizchi/wlt-lang/runtime/simd-kernels/runner.mjs:48)
- JS scalar基準は既にTypedArray for loop。bitmapは各32bitワードのSWAR popcount、bytesは `Uint8Array.indexOf`。f32 dotでは小さい整数値を使い、JS doubleとf32の加算順序差が答えを変えない入力に制約している。単純なArrayメソッド連鎖だけを相手にしていないが、実在JSライブラリのAPI比較でもない。 — [独立したJS基準](/Users/mz/ghq/github.com/mizchi/wlt-lang/runtime/simd-kernels/workloads.mjs:1)
- Core-JS数値listの一括転送後の同じ16,384要素のE2E中央値はsum 22.187µs、dot 41.715µs、bitmap 42.870µs。bytes検索は18.199µs。これでも単発の公開APIはJS基準より遅い、と報告している。r1のJS値とr2のWLT値は別の実行なので、小数点まで正確な新しい速度比として割らない。 — [改善後の比較](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/simd-kernels.md:87)、[r2固定結果](/Users/mz/ghq/github.com/mizchi/wlt-lang/benchmarks/simd-kernels/20261106-r2-bulk/report.json)
- JSON初回のresident parseは40,161 bytes records-512でJSON.parse 58.91µs、WLT 217.48µs。汎用ASTの生成を含む。チューニング同時比較ではWLT residentが220.39→139.80µs、入力＋parse＋serialize全体はCore-JS 520.10→420.74µs、Component 545.44→448.25µsに改善した。後者の測定にはJSON.parseの再測定がないため、二つのrunを混ぜた厳密な新速度比は作れない。 — [初回比較](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/json-parser-bench.md:8)、[変更前後比較](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/json-parser-tuning.md:13)
- JSON数値の原文・重複キーを保存するなどWLTの仕事はJSON.parseと完全に同一でない。速度順位の入力は安全範囲整数・有限小数・重複のないキーに制限する。入力上限65,536 bytes、深さ128で、streaming・ブラウザ・エラー入力の速度評価ではない。 — [比較の契約](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/json-parser-bench.md:50)
- ログ集計は2026-11-04、30,000行・560,000 bytesで25.58ms、JSPI readは30,002回。最終版対初版の交互比較は大部分同等〜数%で、JS実装との比較ではない。在庫台帳も手書き版対マクロ版で12ms台程度、JS Map/BigInt実装は正解oracleであって速度比較対象ではない。 — [ログ計測](/Users/mz/ghq/github.com/mizchi/wlt-lang/examples/apps/log-stats/README.md:69)、[在庫計測](/Users/mz/ghq/github.com/mizchi/wlt-lang/examples/apps/stock-ledger/README.md:94)

### Inferences
- 公開ライブラリとして最有力の形は「一度ロードして多数の照会」「一度の境界呼出しで多数の演算」「小さな集計結果だけ返す」。単発sum/dotの置換は境界費用に負けている。
- `JSON.parse`互換の完全なJS objectを返す汎用parserを第一目標にする根拠は弱い。必要列だけ読む・数値を直接集計・ASTを作らない用途なら別の評価対象として価値がある。ただし同じ出力を作る公平なJS基準を置き、アルゴリズム変更の利益をWLT言語の利益と分ける必要がある。
- 行ごとのread/awaitを減らしてchunk単位に走査し、複数処理を融合することが重要そうである。現在のログ集計の古い数値からPapaParseやcsv-parseに勝てるとは言えない。

### Gaps
- 現行全機能・現行生成物での最新速度値、V8以外・ブラウザでの速度比較、冷起動・p95・GC pause・全Wasm GCヒープの比較はない。既存ABI未解放ブロック0は、全GCヒープの使用量や任意コードの無リークの証明ではない。
- 実在JSライブラリのAPIを使った、同じ意味・同じデータ・同じ出力の比較は未実施。raw jsimdカーネル比較をその代用にしない。

## bitmapの勝因とWLT固有の表現は何か

### Takeaway
WLTはv128をGC配列に直接保持し、i64・popcnt・明示SIMDをWAT相当の精度で書ける。bitmapの差は、ループ構成と還元頻度の違いを含む。effect/ownership解析が自動ベクトル化・noalias・検査削除をしているわけではない。

### Cited Findings
- SIMD例は `array (mut v128)` と、論理長＋配列を持つnominal構造体を使い、pack済みデータをGC参照として保持する。整数・float・bytesを別nominal型にし、内部表現はopaque。 — [GCベクトル表現](/Users/mz/ghq/github.com/mizchi/wlt-lang/examples/simd-kernels/kernels.wlt:23)
- WLT bitmapは `v128.and → i8x16.popcnt → i16x8.extadd_pairwise_i8x16_u → i32x4.extadd_pairwise_i16x8_u` の結果をv128累積器へ加え、最後だけ4laneをi32へ水平還元する。中間bitmapの構築はしない。 — [実装](/Users/mz/ghq/github.com/mizchi/wlt-lang/examples/simd-kernels/kernels.wlt:382)
- 保存済みjsimd側もANDとpopcountを融合したSIMD処理であり、scalarだけではない。ただし `noinline horizontalPopcount(U32x4)` がbyte popcountを水平還元し、各128bitブロックで呼ばれる。 — [捕捉済みZig参照](/Users/mz/ghq/github.com/mizchi/wlt-lang/target/simd-kernels/20261106-r1/reference/bitmap/kernels.zig:45)
- `wasm-tools print target/simd-kernels/20261106-r1/reference/bitmap/kernels.wasm` を今回read-onlyで確認。exported intersection_count（func0）のループには `v128.and; call 1` が残り、func1は `i8x16.popcnt` の後にshuffle/addを4段使って毎回水平還元していた。 — [捕捉済みバイナリ](/Users/mz/ghq/github.com/mizchi/wlt-lang/target/simd-kernels/20261106-r1/reference/bitmap/kernels.wasm)、[元ハッシュ](/Users/mz/ghq/github.com/mizchi/wlt-lang/benchmarks/simd-kernels/20261106-r1/capture.json)
- 現行WLTは線形メモリの `v128.load/store`、splat/laneロード等を型付きvalidator bridgeで受理する。「WLTにv128.loadがない」は誤り。ただしGC packed bytesの `%bytes.get` は `array.get_u $__byte_storage` に展開し、GC byte配列から直接線形メモリのv128.loadはできない。 — [現行許可命令](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/wasm.rs:5)、[GC byte読出し](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/emit.rs:685)
- JSONのGC bytesを16byteずつlaneへ詰める試行は、長いASCIIでは30→24µs程度だが短いキーの多いrecordsは140→167µs程度に悪化し未採用。線形メモリの高速ロードとGC配列は区別すべき。 — [SIMD試行の不採用理由](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/json-parser-tuning.md:42)
- ジェネリクスとnominalなクロージャは単相化し、呼出し先が静的に確定する。emitは直接 `call` を出す。コード生成が同じ関数をインライン化するとまでは保証しない。 — [単相化の契約](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/generics.md:32)、[callbackの契約](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/generics.md:62)、[直接call出力](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/emit.rs:842)
- 現行独自optimizerは、生成直後の共有Pair/Resultをmatchする形をletへ簡約する58行のpass。DCEは別のlink pass。flow/regionsのfactsは検査や割当てを除去する根拠に使っていない。effect検査・所有権検査等はコード生成前に実行する。 — [optimizer全体](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/optimize.rs:1)、[解析と出力の順](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/lib.rs:280)、[factsの用途](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/analysis.md:70)、[全ソース検査](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/lib.rs:154)

### Inferences
- bitmap 5.56倍（対JS）、約2倍（対保存済みjsimd）という数字には、WLTのv128累積→最後に還元する書き方の効果が含まれる。WLT言語そのものやGC対線形メモリの優劣と断定できない。WLT/JS/jsimdで還元構造を揃えるアブレーションが必要。
- WLTならではの狙い方は、低水準WATへ落とせる明示SIMD・i64・単相化と、GCによる複雑な状態の保持を組み合わせること。文字列やグラフを保持する構造の周りに密なnumeric/bitmap arrayを持つ設計に適性がありそう。ただし純粋な連続バッファの単一演算はRust/Zig/Wasmにも可能であり、WLTだけの独占的な性能機能ではない。
- i64の大量内部処理はJSとの境界を出る時だけBigIntにできるため、JS BigInt中心の正確な64bit処理を置換する候補になる。任意精度整数やdecimal演算をi64と比較してはならず、overflow/rounding契約を同じにする。

### Gaps
- effectやsharedが自動noaliasを供給する証拠はなく、現行コードは引数の非aliasを仮定しない。自動fusion・自動SIMD・汎用allocation eliminationもない。ブラウザV8/Wasmtime等のバックエンド最適化と、WLT独自パスを混同しない。
- bitmapの差の原因を定量分離する計測はない。disassemblyは構造差の証拠で、実行時間差の何%を説明するかを証明しない。

## 境界費用と保持APIの最小変更は何か

### Takeaway
まず既存portable APIの一回の呼出しで多数の照会をまとめるのが最小である。呼出しをまたいで任意のゲストGC参照を保持するには、既存raw Coreの仕組みを使う専用JS binding、または未実装のゲスト定義WIT resource adapterが必要。現行公開Core-JS/Component値ABIがそのままresident object APIを持つとは言わない。

### Cited Findings
- Core-JS生成器は `Adapter::validate_boundary()` を通す。portable型はscalar/unit・公開interface型・semantic value等であり、任意の生GC refやv128を受け付けない。 — [境界検査の入口](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/component/core_js.rs:80)、[portable判定](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/component/abi.rs:234)
- bytes/string入力JS codecはstringならUTF-8 TextEncoder、bytesならUint8Array確認の後、ABI線形メモリにコピーする。Wasm側liftは新GC配列を作りbyteごとにコピー、stringならUTF-8を検証しABI領域を解放する。出力もGC→線形メモリ→JS owning copy。 — [JS codec](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/component/core_js/values.rs:84)、[GC buffer lift/lower](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/component/values.rs:117)
- 数値listのTypedArray→ABIはbulk copyに改善済みだが、GCへの変換とSIMD packは別。出力はABIの解放後も独立したTypedArrayコピー。 — [契約](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/lists.md:38)、[codec](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/component/core_js/values.rs:106)
- `$bytes-view/$string-view` のゼロコピーはゲスト内だけ。公開値ABIではbytes/stringへコピー変換する。小さなviewも元buffer全体を保持しうる。 — [viewの境界](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/views.md:55)
- `examples/simd-kernels/resident.wlt` はraw Core GC refを外へ出し、JSに保持させて次の呼出しへ戻す。これは実装済みだがportable値ABIと別API。 — [意図を明記したraw export](/Users/mz/ghq/github.com/mizchi/wlt-lang/examples/simd-kernels/resident.wlt:18)
- native WIT resource実装はimported WIT resourceを検索し、見つからないresourceは拒否する。普通のSDK Core-JS/Component値adapterもtyped own/borrow境界を受け付けない。ゲスト実装のindex resourceが既に提供済みではない。 — [resource検索](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/component/world/resources.rs:20)、[拒否の実装](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/component/world/resources.rs:44)、[公開仕様](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/native-worlds.md:13)

### Inferences
- **最小案A（言語/ABI追加なし）**: `query-many(data, queries) -> small-results` を一つの同期exportとして書き、内部で索引を一度構築して多数のqueryを処理する。既存list/bytes/Result等だけで両portableバックエンドを使える。最初の検証として単発呼出し・中間materializationの問題を切り分けやすい。
- **案B（跨呼出し保持、JS専用）**: raw CoreのGC refを `class Index { #instance; #ref; query(...) {...} }` の非公開フィールドへ保持する専用生成bindingを作る。JS→ゲストは構築時のbulk loader一回、queryは小さな引数、戻り値はcount/ID上位kなどに限定。GCオブジェクトの寿命はJS wrapperが普通に参照する形にし、新たなGC root registryは設けない。同一instanceへの型/brand検査、入力bounds・寿命・例外の回帰検査が必要。
- 案Bで現在のelement setterをN回呼ぶ初期化をそのまま採用すると構築費用が大きい。線形メモリへTypedArray.setした後、typed `v128.load` から `array v128` へ詰めるloaderなら中間のscalar GC配列＋再packを省ける可能性がある。WAT命令は現行対応済みだが、この新しいbulk loader/APIはまだ実装・計測していない。
- **案C（portable resource）**: ゲスト定義WIT resourceの生成と保持adapterを追加。既存のimported resource own/borrowの実装を完成済みのゲストobject APIと扱ってはいけない。案A/Bより大きな作業になる。
- 費用モデルは `T_WLT = load + pack + Σ(call + kernel + result-copy)`、JSは `T_JS = setup + Σ(query)`。照会数Qを変え、構築込みの損益分岐を実測する。r1 residentとr2 single-shot時間の差から「3回で元が取れる」などと推定しない。経路も初期化方法も異なり、現在のresident loadはN回のsetterだからである。

### Gaps
- 通常portable APIで汎用guest GC objectを保持する機能は確認できない。Core-JS readable tableにhandleがあるがFuture/Stream用であり、汎用index保持のAPIではない。
- 案Bのloader・wrapperの実装、cold load、release/GC挙動、参照保持後のメモリピーク、browserのクロスインスタンス拒否は未検証。

## 公平な置換判定と既存資産の再利用

### Takeaway
「明確に置換できる」は手書きJSよりカーネルが速いことではなく、実在ライブラリの同じ公開操作に構築・コピー・結果生成を含めて勝つことで判定する。既存の固定snapshot・oracle・回転測定を再利用できる。

### Cited Findings
- 現行SIMD runnerは入力hash・capture hashを確認、各実装の正解を検査、個別の反復数調整、測定順回転、全値消費、中央値・min/max、ABI live=0の検査をする。 — [測定器](/Users/mz/ghq/github.com/mizchi/wlt-lang/runtime/simd-kernels/runner.mjs:13)、[report生成](/Users/mz/ghq/github.com/mizchi/wlt-lang/runtime/simd-kernels/runner.mjs:108)
- 実行タスクは `just simd-kernels-capture` / `simd-kernels-bench` / `simd-kernels-compare`、JSONにはcapture/bench/compare。新captureや新result出力先を指定し、既存を上書きしない。 — [SIMD再実行](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/simd-kernels.md:106)、[JSON再実行](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/json-parser-bench.md:58)

### Inferences
- 置換判断の**新しい提案基準**（既存の合意KPIではない）: 代表サイズ3段階以上で公開APIのE2E中央値2倍以上、p95悪化なし、構築込み損益分岐のQを明示、最大常駐メモリも競合と同等程度、意味の一致・境界・不正データ検証を維持。2倍は製品判断の仮置きで、実測の事実ではない。
- 比較は (1) 読み込み/構築/pack、(2) resident query、(3) 初回/10回/100回/1000回を構築込み、(4) materialized resultとcount/top-kだけ、を分ける。CPU family/Node/V8/browserとcold/warmを保存する。
- 候補の優先順位の裏付けは、(a) bitmap/facet queryのcount、(b) 大きなresident f32/i64データに対する複数の条件・集計・top-k、(c) bytesから必要フィールドを直接集計する処理、の順で濃い。各JSライブラリの具体名・現在API・相対性能は他の調査ノートを参照する。
- 言語機能を先に増やす必要は薄い。現行の単相化・GC array v128・i64・static call・新しいfor-each/field-pattern/updateはカーネルを書ける。新糖衣は既存IRへ展開するため、それ自体で速度向上を約束しない。

### Gaps
- 2倍という目標や構築回数の採用条件はユーザー未指定。報告で既定KPIに変えてはいけない。
- 今回は既存の結果とコードを監査しただけで、新しいライブラリ置換ベンチは実行していない。
