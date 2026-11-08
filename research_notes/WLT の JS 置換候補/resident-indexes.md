# 常駐インデックスを使うJSライブラリの置換候補

調査日: 2026-11-08。外部資料は公式リポジトリのREADME・ソースをGitHub connectorで取得した。新たな速度計測は行っていない。下記の速度は既存の2026-11-06計測だけであり、候補ライブラリへの勝利を確認した数値ではない。

## 1. Bitmapの絞り込み・件数・ランキングは最有力か

### Takeaway

最初に狙うのは、ブラウザ内のファセット検索・一覧絞り込みで使う、常駐する複数bitmapのAND/OR/NOTと件数取得。単発の二集合交差だけでなく、条件合成と複数の集計を同じ走査へまとめ、JSへ小さな結果だけ返す用途がよい。まずpure JSに対する用途限定のE2E優位を検証し、Roaringの既存Wasm/ネイティブも対照にして上限を知る。

### Cited Findings

- FastBitSetは整数集合のunion/intersection/differenceに加え、集合を作らず件数だけ求める`intersection_size`、存在判定の`intersects`、in-place操作を提供している。作者自身もin-place操作の割当て削減を勧める。比較側を常に`new_intersection(...).array()`にするのは不公平。[FastBitSet API](https://github.com/lemire/FastBitSet.js/blob/master/README.md)
- 現行`intersection_size`の本体は最小ワード長まで走査し、ANDした32bit値をSWARの`hammingWeight`で加算する。`intersects`は最初の一致で終了する。現行ソースの内部`words`は通常のArrayで、`fromWords`も提供される。README冒頭の「typed arraysを活用」のような一般文だけから格納を断定しない。[FastBitSet実装](https://github.com/lemire/FastBitSet.js/blob/master/FastBitSet.js)
- FastBitSetのREADMEはNode用途の高性能・省メモリ対照としてroaringを挙げている。roaring-wasmは圧縮bitmapを扱い、Node/Browser/Deno向けのWasm実装。作者はNode専用のroaring-nodeがネイティブSIMD等を利用できると説明している。この作者の比較主張をWLTとの実測結果へ置き換えない。[FastBitSet](https://github.com/lemire/FastBitSet.js/blob/master/README.md)、[roaring-wasm](https://github.com/SalvatorePreviti/roaring-wasm/blob/master/README.md)
- 既存WLT bitmapカーネルはGCの`array v128`を保持し、`v128.and`、`i8x16.popcnt`、拡張pairwise加算で件数を求める。[WLTカーネル](/Users/mz/ghq/github.com/mizchi/wlt-lang/examples/simd-kernels/kernels.wlt:382)
- Apple M5 / Node24.21 / V8 13.6、9標本の既存計測では、16,384ワードの常駐二集合交差件数がJS SWAR 16.000µs、WLT SIMD 2.878µs（5.56倍）。これはFastBitSetパッケージの公開APIとの比較ではない。一括転送改善後も、毎回入力するbitmap SIMDは42.870µsであり、保持・再利用の前提が決定的。[既存測定](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/simd-kernels.md:55)

### Inferences

- 第一候補の製品形は「検索UI用の小さな常駐フィルターエンジン」。初期投入後は条件IDを数個渡し、count/facet-counts、必要なら上位k件のIDとスコアだけ返す。全bitmapや対象レコードを毎回ABI越しに返さない。
- 例えば`count((A OR B) AND C AND NOT D)`をワードごとに評価してpopcountへ流す。逐次の集合API呼び出しに伴う中間集合と複数走査を減らせる。ただし同じ融合は手書きJSでも可能なので、最適化したUint32Array＋SWARの融合JSも必須対照とする。言語差とアルゴリズム差を分離する。
- countの後に全IDを列挙してからJSでsortする方式より、WLT内で条件評価・必要なスコア処理・top-kを完了させる方が境界費用を小さくできる可能性がある。top-kのランダムなスコア参照や疎な列挙は別のボトルネックになり得るため、countからの速度比をそのまま外挿しない。
- 高密度で比較的狭いID空間ならpacked bitmapが候補。非常に疎な集合では全ワード走査が不利になり、ソート済みposting listやRoaringの疎密切替が有利かもしれない。疎密適応はデータ構造の工夫であってWLTが自動的に得る利点ではない。
- GCの主な貢献は木・辞書・メタデータとpacked配列の参照寿命を一緒に扱えること。割当てを行わないbitmapのhot loopで「Wasm GCだから速い」とは言えない。SIMD・表現・走査の融合による差とGC負荷を分ける。
- 新しい言語構文は不要。まずライブラリとAPI、密度別の表現選択を試す。自動fusionをeffect解析が既に行うと説明しない。

### Gaps

- FastBitSet、最適化した融合JS、roaring-wasm、roaring-nodeに対するWLTの同条件E2E計測はない。既存Wasmを上回れるという根拠もない。
- 常駐データの初期コピー・pack・構築費用を何クエリで償却できるかは未測定。既存residentは初期setter転送を除外している。
- 提案するJSクラス風APIを現在のportable Core-JS/Component adapterがそのまま提供できるとは確認していない。実験のraw Core GC参照を通す経路はWIT値ABIと別。[境界の区別](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/simd-kernels.md:47)

## 2. 常駐ベクトルの絞り込み付きtop-kは第2候補か

### Takeaway

小〜中規模のベクトル集合をCPU内に保持し、metadataのbitmap条件で候補を絞ってcosine/top-kを返す処理は第2候補。単一の内積関数を移すより、反復利用と複数処理の一体化でAPI全体を狙う。ただし精度・アルゴリズム・GPUとの比較を揃えない大きな速度比には価値がない。

### Cited Findings

- `ml-distance`はJSから使うベクトル距離・類似度のライブラリ。現行`similarity.cosine`は内積と2つの二乗ノルムを一つのループで計算する。古い実装の3回走査だけを比較対象に選んではならない。[API](https://github.com/mljs/distance/blob/main/README.md)、[cosineソース](https://github.com/mljs/distance/blob/main/src/similarities/cosine.ts)
- 既存WLT resident内積は同条件のJS 10.349µsに対し2.537µs（4.08倍）。しかしpackを毎回行う版は11.126µsで、比較用入力は加算順の違いが出ない小整数だった。一般の浮動小数点値や検索品質の保証へは外挿できない。[既存測定と条件](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/simd-kernels.md:51)
- VoyはRust/Wasmのvector searchで、インデックスを保持するクラスとFloat32Arrayのqueryからtop-kを返すAPIを既に持つ。Wasmでベクトルを処理するだけでは差別化にならない。[Voy API](https://github.com/tantaraio/voy/blob/main/README.md)
- TensorFlow.jsにはWebGPUのGPUバックエンドが存在する。GPUを使う既存ライブラリに対してCPU SIMDの内積だけで優位と決めつけない。[TensorFlow.js WebGPU](https://github.com/tensorflow/tfjs/blob/master/tfjs-backend-webgpu/README.md)

### Inferences

- 製品仮説は`filter bitmap -> candidate vector dot/cosine -> bounded top-k`を同じguest呼び出しにまとめること。ベクトル全体を毎回渡さず、queryベクトルと条件だけ入力し、k件のみ出力する。文書・タグ等のGCメタデータと数値blockの組合せがWLTの扱いやすさに合う。
- ノルムの前計算はJS側にも同じ条件で許す。plain JSの各候補に`ml-distance`を毎回呼ぶ版だけでなく、事前正規化＋融合されたTypedArray走査を対照にする。
- WLT f32のSIMD加算とJS Numberの逐次f64加算を完全同一の計算と呼ばない。出力誤差、同点順、ゼロベクトル・NaN/Infinityの扱い、top-kの境界を先に契約化する。近似ANNと全件exact探索を速度だけで比較しない。
- ノードを大量に追うHNSW等ではCPUキャッシュ・ポインタ参照が支配する可能性があり、GCモデルだけで勝利を予測しない。最初はexactの限定用途で検証する。

### Gaps

- フィルター付きtop-kの新実装・速度・精度は未測定。WLTがVoyやWebGPUを上回る根拠はない。
- 次元数・候補数・クエリbatch数によるCPU/GPU切替点、GC配列配置のcache効果は未測定。

## 3. 全文・空間索引の優先度と、勝利を判定する実験

### Takeaway

全文検索・空間索引の丸ごとの移植は後回し。成熟JSライブラリの仕様・レイアウトを再実装する費用が大きく、GC化だけの利点は弱い。最初の判断材料はbitmapの限定APIで「初期構築を含む実運用の総時間」と「クエリ応答・メモリ」を測ること。

### Cited Findings

- Flatbushは静的なpacked Hilbert R-treeで、単一ArrayBufferに索引を格納し、Worker間のtransferやSharedArrayBufferを受け付ける。searchだけでなく最初の一致で終了するcollidesも持つ。これをGC木へそのまま移しても既にあるコンパクト性・transfer機能を自動的に得るわけではない。[Flatbush API](https://github.com/mourner/flatbush/blob/main/README.md)
- RBushは更新可能なR-treeでbulk insertion、search、collides等を提供する。静的Flatbushとの速度差を言語差として測定しない。[RBush API](https://github.com/mourner/rbush/blob/main/README.md)
- MiniSearchはprefix/fuzzy/ranking/field boostを持つin-memory全文検索。FlexSearchもtokenizer/encoder、複数field・tag検索、Worker、永続化等を持つ。ASCII単語の転置索引やposting交差だけでこれら全体を置換したとは言えない。[MiniSearch](https://github.com/lucaong/minisearch/blob/master/README.md)、[FlexSearch](https://github.com/nextapps-de/flexsearch/blob/master/README.md)
- WLTの具体化したclosure型を使うmap/foldは直接callになるが、mapの出力は作業GC配列からlistにコピーする。現時点の`src/optimize.rs`は共有Pair/Resultの生成直後matchをletへ簡約する小さなpassで、一般のfusion・自動vectorize・effect由来のloop hoistingを実装しているわけではない。[closureとコピー](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/generics.md:62)、[現行optimizer](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/optimize.rs:15)
- portable数値listのTypedArray転送は一括コピーだが、線形メモリとGCの変換・SIMD packは残る。返却値も独立したコピーになる。[ABI契約](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/lists.md:30)

### Inferences

- 空間索引なら、まず構築済みindexを常駐させ、bbox検索＋属性条件＋件数・集約をまとめる限定APIが候補。JS callbackを候補ごとに呼ぶ設計は避ける。ただしFlatbushもfilterFnを持ち、JS JITのinline可能性もあるため、WLTの直接closure呼び出しが常に優位という主張はしない。
- 全文ならtokenizer・Unicode正規化・fuzzy仕様を固定できる業務検索に絞り、posting交差＋スコアリングを先に狙う。日本語検索全体を「UTF-8なので高速」と売り込まない。
- 最初のbitmap実験の提案: ID空間1万/10万/100万、密度0.01%/1%/10%/50%/99%、均一・clustered・相関条件。3〜8条件のAND/OR/NOT、count、facets、上位20件、全ID列挙を別々に測る。更新なし/小batch更新を分け、packed状態再構築の費用も入れる。これらは試験案であって実測値ではない。
- 対照は(1)FastBitSet公開APIの適切なin-place/scratch再利用、(2)同じ融合アルゴリズムの最適化TypedArray JS、(3)roaring-wasm、Nodeで可能なら(4)roaring-node。疎なケースではsorted posting listも対照にする。まず(1)(2)への用途限定E2E優位を検証し、(3)(4)は既存Wasm/nativeと比べた上限・劣位も公開する。
- 初期転送・構築・compile/instantiateを計上するcold系列と、常駐queryのwarm系列を分ける。Q=1/10/100/1000の総時間 `Tinit + Q*Tquery` を比較し、break-evenは実測から算出する。Qが何回なら償却できるとはまだ言わない。メモリはJS heapだけでなくGC/ArrayBuffer/Wasm memory/一時データ保持とpeakを調べる。
- 提案する採用基準は、あらかじめ決めた主要負荷でE2E中央値が2倍以上、p95にも改善があり、意味上の答えが一致し、初期時間・メモリ増分に許容範囲があること。これは目標案であり達成済みの数値ではない。小さな差だけなら既存JSを置換する維持費を正当化しにくい。
- 「WLTならでは」は、WATに近い明示的なSIMD/整数操作とGCで管理するメタデータを、小さい静的型・単相化・effect契約の同じプログラム内で扱える点。SIMDそのものは他のWasm言語でも使える。GC・所有権・effect検査が性能優位を自動的に証明するわけではない。

### Gaps

- ここで挙げた外部ライブラリとの新しい実運用比較は行っていない。著者のREADMEの速度ランキングは、対象負荷・エンジン・意味が揃わないためWLTの順位根拠には使っていない。
- 新しい言語機能を加える必要性は現段階で見つかっていない。汎用resident公開APIとbulk→packed構築の扱いは、既存のraw Core経路とportable経路を区別して別途設計・測定する必要がある。
