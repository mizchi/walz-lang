# 公開後不変の共有領域

`%race-free`の固定atomicセル契約を、読み取り専用の範囲とchecked loadへ拡張する。共有前に初期化が完了し、公開後はホストも内容を変更しないという前提の下で、任意のエージェント数・長さのtraceにデータ競合がないことをLeanで検査する。

```wat
(module
  (%race-free)
  (import "env" "memory" (memory $shared 1 1 shared))
  (%atomic.cell $cancel $shared i32 0)
  (%readonly.region $index $shared 4096 8192)
  (func $read (export "read") (param $offset i32) (result i32) (%effect reads)
    (%readonly.load $index i32 $offset)))
```

`(%readonly.region $name $memory byte-offset positive-byte-length)`は初期メモリ内の固定範囲を宣言する。`(%readonly.load $name u8/i32/i64/v128 relative-byte-offset)`は、その範囲内から1・4・8・16byteを読む。u8の結果型はi32。アドレス式を一度だけ評価し、符号なしで`relative <= length - width`を検査してからロードする。i32のビットパターンは符号なしoffsetとして扱い、論理範囲を越えるアクセスはtrapする。Wasm全体のメモリ内にあっても、宣言領域を越えれば拒否する。

非atomicロードなので、アドレスの自然アライメントは要求しない。`reads`を持ち、暗黙の同期・停止・型変換は追加しない。通常の`i32.load`や`v128.load`をこのモードで使っても、検証を迂回できない。

ファイルでは`(%export (region $index))`と`(%use "./layout.wlt" (region $index $alias))`を使える。region名は値・整数ポインタとして取り出す型ではなく、静的なロードの指定先である。

## 構造検査と独立カーネル

readonly領域を持つ`%race-free`のモジュールでは次を検査する。

- 共有元は固定limitsのshared memory一つ。共有メモリのraw exportとホスト関数importを拒否する。
- 領域は正の長さ、初期メモリ内、最大256。atomic制御セルと重ならない。readonly領域同士の重なりは許す。
- 共有payloadの通常アクセスはchecked readonly loadだけ。書込み・raw load・memory.copyによる共有先への書込みを拒否する。
- Worker自身が定義するunshared memoryは私有として扱い、通常のロード・ストア・grow・exportを許す。私有メモリをimportすることは許さない。
- ABIの公開関数は数値引数・数値/unit結果。GCオブジェクトはエージェント内に留める。

証明書version 2にはatomicセルとアクセスに加え、readonlyRegionsとreadonlyAccessesを含める。従来のversion 1の規則・証明は残す。カーネルはロードopcodeのwhitelist、幅、領域番号、範囲、制御セルとの非重複を独立に検査する。[AtomicCells.lean](../proof/AtomicCells.lean)のReadonly名前空間が、許された動的アドレスのロードを含むtraceの`race_free`と`readonly_in_bounds`を証明する。偽造したstore・幅・領域番号・重なり・versionを拒否するテストを持つ。

私有メモリとGC操作は共有payloadのtraceから除く。Wasmエンジンのメモリ分離、抽出器、unsigned guardとloadの出力、初期化・公開の順序、ホストの契約順守はTCB／前提に残る。Builderの静的なfreeze所有権、任意のJSコード、Workerスケジューラ全体を証明したものではない。

## ABIとホスト契約

Core＋JSPI成果物は`wlt-core-js-v4`と`wlt-readonly-regions-v1`を使う。旧runtimeが新しい契約を無視して実行しないためのversionで、値codecは既存のものを使う。

```js
const { api } = await instantiate({
  imports: { env: { memory } },
  readonlyHostContract: 'wlt-readonly-regions-v1',
});
```

ホストは公開前に初期化し、公開後はreadonly領域へ書かない。制御セルへは同じ幅のAtomicsだけを使い、全Workerの終了確認前に領域を再利用しない。指定文字列はJSコードの検証やOSのメモリ保護ではない。raw Coreを直接instantiateする場合も同じ前提が必要。

Core＋JSPIのhost契約handshake、論理範囲末尾・負offset、アドレス式の評価回数、偽造証明書、実際のWorkerでの処理を`just test-succinct`で検査する。Componentでのshared memory import/exportは引き続き対象外。並列SDKはCore Workerを使い、単体のprivate-memory APIはCore＋JSPI・Componentの両方に出力する。

`%race-free`なしではregionの宣言とchecked loadは使えるが、公開後不変や競合排除を検証済みとは扱わない。通常モードの書込みを禁止する構文ではない。
