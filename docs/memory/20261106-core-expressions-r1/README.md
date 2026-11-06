# 5つの型付きコア拡張のメモリ検査

測定日時: 2026-11-06T02:40:13.324Z。Node v24.21.0 / darwin arm64。

数値命令、result.do、スカラーnewtype、早期脱出、所有値と例外の併用を対象とした。通常のCore Wasmと、Core＋JSPI / Componentの境界を実行した。

## 結果

| Backend | ケース | GC後のJS heapUsed（最初 → 最後） | 増減 |
|---|---|---:|---:|
| core | Result / return / br_if / owned例外 | 5.752 → 5.807 MB | +54.2 KiB |
| core | newtype list / Future / Stream | 5.922 → 5.998 MB | +74.7 KiB |
| core | using・全終了経路 | 6.349 → 6.412 MB | +61.0 KiB |
| component | Result / return / br_if / owned例外 | 7.642 → 6.519 MB | -1096.3 KiB |
| component | newtype list / Future / Stream | 7.839 → 7.873 MB | +33.2 KiB |
| component | using・全終了経路 | 8.892 → 8.851 MB | -39.5 KiB |

- 全36サンプルで線形メモリは64 KiB、未解放ABIバッファ、配送端点・task/thread/subtaskの表、外部resource、待機中のread・blocking・disposal、task queueはゼロ。
- ArrayBufferの保持量は全ケースで最初と最後の差が0 bytes。JS heapUsedの増加は各ケース2 MiB未満という検査を通過した。
- 各backendで11,200個の資源を取得し、11,200個をcloseした。各448個の資源オブジェクトをWeakRefで追い、instanceを生かしたまま全て回収された。
- Core Wasmでは、7経路×32回のBoxと内側のGC array、計448オブジェクトの回収を直接観測した。Wasmのmutable fieldから保持する対照例は回収されず、fieldをclearした後に回収された。
- return、br、取消し、trap、destructor失敗、list引数の部分的な変換失敗について、終了済みtask・error・producer Promiseを保持し、不要なWasm Memoryと資源オブジェクトが回収された。

初回の観測ではComponentの変換失敗後のMemoryが残ったが、履歴を保持したまま次の観測で回収された。単独実行とheap snapshotでも持続する保持経路は得られなかった。WeakRefのderefはそのjobの間だけ対象を生かすため、生成と観測を分離し、job境界を跨いで最大8観測（各4回のGC要求）で確認する。今回、履歴の回収はCoreが1観測、Componentが2観測で完了した。ランタイムの修正はしていない。

## 負荷と再実行

各ケースはwarmup 1バッチの後、6バッチを測定した。JSのビルドツールは別プロセスで実行し、測定プロセスへ読み込まない。

1. Result / 早期脱出 / owned例外: 1バッチ400反復。1–32 KiBのbytesを使い、成功・先頭err・後続err、return、br_ifの両方、通常経路とcatchを通す。数値変換とsqrtも毎回実行する。
2. newtypeの配送: 1バッチ200反復。16–512要素のi64/newtype list、Futureの返却・await、Streamの返却・完走・途中closeを通す。
3. 外部資源: 1バッチ100反復、1反復16資源。normal / Result.err / explicit close / nested / catch / return / br / br_if両方 / 内側のblock / recurを通す。資源のpayloadは16 KiBのUint8Array。二重close、close後のreadも拒否する。

```sh
just memory-core-expressions
# 別の出力先へ保存する場合
just memory-core-expressions target/core-expressions-memory/recheck
```

終了を強制せず、タイマーや未完了のタスクがプロセスを残す場合も検出する。`__memory`と`__taskMemory`は生成後の検査成果物にだけ挿入する。実行時ライブラリや通常出力に登録表を追加しない。

測定値・生成物と検査用生成物のSHA-256・ソースのSHA-256は [report.json](report.json) に固定した。元の実行成果物は `target/core-expressions-memory/20261106-r1/`。凍結したこのreportは上書きせず、新しい負荷や実装では新しいrunを作る。

既存のGC・ABIアリーナ・配送・Coreの失敗履歴・資源解放・新しいABIの回帰検査も、`node --expose-gc --test runtime/memory.test.mjs runtime/retention.test.mjs runtime/core.test.mjs runtime/resources.test.mjs runtime/language-core.test.mjs` で15件成功した。

## 確認できた範囲

この負荷と終了経路では、持続的な参照保持や解放漏れは観測しなかった。GC後の到達性、管理表の残数、ABIアロケータの再利用、反復後の保持量を照合した結果であり、任意のプログラム・engine・host producerに対する無リークの証明ではない。

RSSは途中で増えても、GC heap・Wasm Memoryの生存数・未解放項目とは区別する。今回のRSSも後半で安定したが、エンジンの確保済み領域がOSへ即時返却されることは検査条件にしていない。呼出し元が保持する結果、未消費のFuture/Stream、hostが意図的に保持する資源は検査の対象外。
