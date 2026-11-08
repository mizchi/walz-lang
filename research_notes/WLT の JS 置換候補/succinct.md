# jsimdを参考にした簡潔データ構造と並列処理

調査日: 2026-11-08。ここでの「簡潔データ構造」はsuccinct data structures（rank/selectを基礎とするbitvector、wavelet matrix、Elias–Fano、LOUDS等）の意味で扱う。新たな実装・計測は行っていない。取得したjsimd mainのcommitは `2cf51e8885314764dcb44b6469899d95f204b148`。参照は以下すべてこのcommitに固定する。記載する既存速度はjsimd側の公開済み実験条件に限られ、WLTへ移せば同じ速度が出るという意味ではない。

## 1. jsimdで実装済みのものと、WLTの最小の実装順

### Takeaway

基礎は「構築中だけ可変なpacked bitvector → freeze → immutableなrank/select索引」。ただし単発rank/selectの大きな加速を売りにせず、その上のwavelet matrixによる範囲頻度・分位点という、通常の配列走査やcopy-sortを置換する用途を製品の入口にする。実装順はbitvector/rank/select → 小さい値域のwavelet matrix（まずu8相当）→ read-only query batchのWorker分配。Elias–FanoとLOUDSはその後。

### Cited Findings

- jsimdには公開済みの`RankSelectBitVector`、`WaveletMatrixUint8/16/32`、`EliasFanoSequence`がある。未索引の`BitVector` / `bit-vector`サブパスは予約済みで未export。これらをjsimdで未実装と説明してはならない。[package API](https://github.com/mizchi/jsimd/blob/2cf51e8885314764dcb44b6469899d95f204b148/packages/jsimd/README.md)
- rank/selectは不変snapshot、rankは半開区間、selectは0-originで欠落時−1、bulk APIは一括入力・再利用可能な出力を持つ。READMEの既存計測ではindexed JSと同じ512bit索引を比較し、single rankはJSより遅いケースがある。これは基礎機能の実用性を示すが、ランダムな1回の問い合わせでSIMD優位を保証しない。[RankSelectBitVector APIと測定](https://github.com/mizchi/jsimd/blob/2cf51e8885314764dcb44b6469899d95f204b148/packages/jsimd/src/rank-select-bit-vector/README.md)
- `WaveletMatrixUint32`は各bitlevelの安定な0/1 partitionとrank索引を使い、access/rank/select/rangeFreq/quantile/predecessorを持つ。32levelのbit列とrank metadataで約4.25 bytes/value＋小さな境界情報となり、元のUint32Arrayの4 bytes/valueより必ず省メモリになる構造ではない。構築は32passで、追加の作業領域も持つ。[wavelet API・layout](https://github.com/mizchi/jsimd/blob/2cf51e8885314764dcb44b6469899d95f204b148/packages/jsimd/src/wavelet-matrix-uint32/README.md)
- 同READMEの既存実験ではquantileがcopy-sortより大幅に速い一方、direct accessはTypedArrayが約100〜150倍速く、exact rankも専用の`Map<value, sorted positions>`が強い。波形行列全APIがJSより速いわけではなく、複数種類の範囲・順序統計を一つの索引で処理する用途に限る。[wavelet測定条件と負けるケース](https://github.com/mizchi/jsimd/blob/2cf51e8885314764dcb44b6469899d95f204b148/packages/jsimd/src/wavelet-matrix-uint32/README.md)
- Elias–Fanoは重複を許す非減少u32列で、packed low部分とunary high部分を持つ。jsimdはpoint access、rank、nextGEQ、predecessor、partitioned版を実装し、decodeと単純point accessにはTypedArrayが強いと明記する。[EF契約](https://github.com/mizchi/jsimd/blob/2cf51e8885314764dcb44b6469899d95f204b148/packages/jsimd/src/elias-fano-sequence/README.md)
- LOUDSは公開モジュールではなく棄却済み実験。87,381-nodeの4分木で4096親取得を比較し、batched select+rankは0.0758ms、直接Uint32Array親表は0.0113ms（直接表が6.70倍速い）。[棄却理由](https://github.com/mizchi/jsimd/blob/2cf51e8885314764dcb44b6469899d95f204b148/experiments/louds-topology/README.md)

### Inferences

- 最小の公開契約は、`MutableBits`を作って`freeze`で使えなくし、`FrozenRankIndex`だけから`rank1/rank0/select1/select0`とbatch版を呼べるもの。実際のWLT所有・共有capabilityは別途型検査の実装と照合する必要がある。GCオブジェクトを単にimmutable structにしただけでホストの共有メモリの書換えまで禁止できるとは言わない。
- 最初の置換領域は「タイムレンジを指定して価格・スコア・カテゴリIDのrangeFreq / quantileを何度も求めるクライアント側分析」。TypedArray slice/filter/sortや一般JSの範囲集計を、事前構築した索引に置き換える。これはアルゴリズム上の利点が中心で、WLT固有の速度だけではない。同じwavelet layoutを使うJSと既存jsimdも対照にする。
- u8相当から始めれば8levelで済み、32level版より実装・構築・問い合わせ・並列時のbarrierを小さくできる。WLTにconst generics等の新構文を先に加える必要はなく、固定widthの実装または検査したwidth引数で試せる。WLTのi32が自動的にu8の値域を保証すると扱わない。
- 速度を最初に出しやすい隣接用途はbit-sliced columnである。predicate → mask合成 → countを同じshared線形メモリ内に置けば、queryごとのID列挙とコピーを避ける道筋がある。ただしjsimd自体が既にこの設計を採用しており、そのコピーを越えるとは未確認。[jsimd bit-sliced API](https://github.com/mizchi/jsimd/blob/2cf51e8885314764dcb44b6469899d95f204b148/packages/jsimd/src/bit-sliced-column/README.md)
- 独立featureごとにWasm memoryが分かれるjsimdでは、bitmap→rank/selectのfreeze橋渡しはコピー、waveletは別rank-selectモジュールを呼ばず同一モジュールにrank処理を埋め込む。WLT側では必要な部品を静的linkして同じmemory/契約を共有できるライブラリ構成が候補になる。ただしzero-copy化には所有権移転・不変化・storage layoutの実装が必要で、言語が自動的に達成するわけではない。[bridge契約](https://github.com/mizchi/jsimd/blob/2cf51e8885314764dcb44b6469899d95f204b148/packages/jsimd/src/rank-select-bit-vector/README.md)、[wavelet内のrank埋込](https://github.com/mizchi/jsimd/blob/2cf51e8885314764dcb44b6469899d95f204b148/packages/jsimd/src/wavelet-matrix-uint32/README.md)

### Gaps

- WLT側のrank/select、wavelet、EF、LOUDS標準ライブラリは今回の`stdlib`・`src`・標準API一覧の検索では見つからなかった。実装提案であり既存機能ではない。
- 固定512bitごとのu32索引は実用上コンパクトでも、補助領域が固定比率なので数学的な厳密さでいうn+o(n)のsuccinct保証と同一ではない。「簡潔」の用語を厳密なentropy-boundの達成済み主張にしない。

## 2. READMEと実装の不一致、およびrank/selectの正しさで先に固める点

### Takeaway

jsimdのAPI・ベンチ方針は良い参考になるが、現commitのrank/select READMEには実装と一致しない記述がある。特にselectの計算量とmetadata比率はコードから取り直す必要がある。WLTへ移植する際には、定義と境界条件を独立したscalar referenceで検査することが性能比較の前提。

### Cited Findings

- READMEの73〜76行は512データbitごとにu32 prefixを置く一方、metadata比率を0.78%とし、select1がprefixを二分探索して最大16wordを走査すると説明している。[README L73](https://github.com/mizchi/jsimd/blob/2cf51e8885314764dcb44b6469899d95f204b148/packages/jsimd/src/rank-select-bit-vector/README.md#L73)
- 実際の`select`は36行から、先頭wordからpopcountを減算して走査する。`select1`（54〜58行）も`select0`（59〜63行）もindexとsuperblocks引数を明示的に捨て、同じ走査関数を呼ぶ。batch版も各queryについてこれらを繰り返す。したがって現ソースでは最悪時は全word走査であり、「最大16word」は成立しない。[kernel L36](https://github.com/mizchi/jsimd/blob/2cf51e8885314764dcb44b6469899d95f204b148/packages/jsimd/src/rank-select-bit-vector/kernels.zig#L36)、[select1/0 L54](https://github.com/mizchi/jsimd/blob/2cf51e8885314764dcb44b6469899d95f204b148/packages/jsimd/src/rank-select-bit-vector/kernels.zig#L54)
- TypeScript側はword数を16で割ったsuperblock数を使い、`(superblocks + 1) * 4` bytesのrank領域を確保する（90〜93行）。よってデータ512bit=64bytesあたりprefix4bytes、漸近的な比率は4/64=6.25%である。端点・padding・allocator roundingは別に増える。0.78%は算術上の不一致。[allocator metadata L90](https://github.com/mizchi/jsimd/blob/2cf51e8885314764dcb44b6469899d95f204b148/packages/jsimd/src/rank-select-bit-vector/mod.ts#L90)
- wavelet側は別のkernel実装を使い、selectBitはlevel上の位置をrankで二分探索する（153行から）。単独RankSelectBitVectorのselect実装と同一の経路ではない。[wavelet selectBit](https://github.com/mizchi/jsimd/blob/2cf51e8885314764dcb44b6469899d95f204b148/packages/jsimd/src/internal/wavelet_kernel.zig#L153)

### Inferences

- WLT版のselectでは`P[j] <= target < P[j+1]`になるsuperblockを選んで、そのblockのみを読む改善を試せる。ただし空superblockでprefixが重複する。単純な「一致した位置」を返すbinary searchは不適切で、対象が総数未満という事前条件の下で`upper_bound(P, target) - 1`相当を使う必要がある。
- select0用のprefixは`min(j*512, logicalLength) - P[j]`とする。paddingをzeroと数えると末尾のselect0が範囲外を返す。最後のwordをlogicalLengthでmaskし、空・全zero・全one・block境界・未満/同値/上限・欠落queryを必ず検査する。
- rankは`rank1(end)`が[0,end)で、end=0とend=Nを許す。selectは0-originで`target >= count`なら欠落。`rank1(select1(k)+1)=k+1`、`rank0(end)=end-rank1(end)`、builderからfreeze後に元を更新しても結果不変、といった関係をproperty/differential検査にできる。WLT lawの現在の証明断片がGC/線形memory全体を証明できるとは仮定せず、契約モデルと実コード検査の範囲を区別する。
- この不一致はjsimdリポジトリへの変更依頼ではないため、コード・README・既存ベンチsnapshotを修正していない。性能値がどの生成Wasmを使ったかも今回新たに検証していない。

### Gaps

- README測定に使ったWasmと現sourceの一致は未確認。selectのベンチ値をそのまま新WLT版の基準値と見なさず、公開APIと生成物hashを固定して再測定する必要がある。

## 3. マルチスレッド化する単位と、実用的な検証順

### Takeaway

最小で扱いやすい並列契約は、shared線形memoryに一度構築したpacked indexをfreezeして全Workerから読み、query batchと出力領域だけ分割する形。各WorkerのWasmGCメタデータはローカルに置く。可変索引のconcurrent mutationやGCヒープ共有を最初から狙わない。

### Cited Findings

- jsimdにはshared memory/Workerの実装とexact vector searchのsharding例がある。ただしこの例では各Workerがprivate Wasm indexへ担当rowをコピーし、共有はqueryと通知・結果slotに使う。packed indexそのものを各Workerが同じ場所から読むimmutable共有設計とは異なる。[example構成](https://github.com/mizchi/jsimd/blob/2cf51e8885314764dcb44b6469899d95f204b148/examples/multithread-vector-search/README.md)
- 同例の既存測定はM5/Deno2.6.4/128次元/4Workers/20常駐queryで、32K vectorsでは単一版0.81msに対して4Worker2.50msと負け、131Kでほぼ同等、262K〜524Kで約2.1倍。初期化とコピー、coordinator mergeの費用がある。これをWLTの期待値や一般的な4倍速の根拠にしない。[既存parallel測定](https://github.com/mizchi/jsimd/blob/2cf51e8885314764dcb44b6469899d95f204b148/examples/multithread-vector-search/README.md)
- wavelet構築は前levelのpartition結果を次levelへ渡す逐次level構造である。現在のjsimdは各levelのbit生成・rank構築・stable partitionを順に実行する。[build実装 L32](https://github.com/mizchi/jsimd/blob/2cf51e8885314764dcb44b6469899d95f204b148/packages/jsimd/src/internal/wavelet_kernel.zig#L32)

### Inferences

- 最初は同じアルゴリズムの1Workerと2/4Workersでquery batchを比べる。rankやquantile一回ごとにWorkerへmessageを送らず、一つのbatchを大きな連続query範囲に分割し、各Workerが専用output範囲へ書く。immutable indexを読むだけならpayloadのatomic read-modify-writeは不要にできる設計が候補だが、現WLTのrace-free検査がこの権限を認めるかは別の型・host契約の作業。
- rank索引構築のparallel案は、各chunkでpopcountの局所prefixを作り、chunk総数の小さなprefix sumを計算し、chunkごとにoffsetを加えてpublishする3段階。各構築領域を排他的に分け、freezeを全writer終了後の一方向状態遷移にする。
- wavelet構築も各levelのbit分類・chunkごとのzero/one数・prefix sum・安定なdisjoint scatterは並列化できる。ただしlevel間は依存し8/32回のbarrierが生じる。まずquery batch並列化、その後build並列化が小さい実装順。
- shardごとのrangeFreq/countは足し合わせられるが、global quantileを各shardのquantileの平均や中央値で求めることはできない。global quantileには追加のvalue-domain探索やrankの集約が必要で、単純shardingの売り文句に含めない。
- bitmapの大きな連続走査はmemory bandwidthで、rank/waveletの依存するrandom readはmemory latency/cacheで頭打ちになる可能性がある。SIMD倍率×Worker数の倍率になると想定せず、1Worker SIMDを必須対照にする。
- 測定は(1)indexed JS、(2)jsimd現行API、(3)WLT同layout single Worker、(4)WLT shared frozen index＋query batchの複数Worker。build/transfer/freeze/startup、Q回query、結果copy、保持量/peakを分ける。全bitvectorの連続アクセスだけでなくランダム/偏ったquery、疎/密、tail、zero/one連続runを含める。境界償却回数は実測未定。
- 最初の契約はshared memory全体の外部aliasまで制御しなければ条件付きである。JS側に残したwritable TypedArrayからfreeze後に書かれた場合までWLT内部の型検査だけで防げるとは主張しない。所有移転・Worker終了・reader寿命・取消し時のbuffer再利用についてはruntime/type-checkerの担当調査と統合する。

### Gaps

- WLTでこのshared frozen regionの型付き公開API・Workerpool・取消し契約が既に使えるとは今回確認していない。実装前に現行threads/race-freeモデルとの接続を設計する必要がある。
- WLT版の実測結果、jsimdに対する速度差、query batchの必要最小サイズ、memory bandwidthの飽和点は未測定。追加言語機能の有無は型付きregion契約の最小化を先に検討して判断する。
