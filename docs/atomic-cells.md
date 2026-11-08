# 固定 atomic セルの条件付き race-free 検証

`%race-free` を付けたモジュールでは、共有線形メモリへのアクセスを静的に宣言した atomic セルに限定する。Rust の型・所有権・effect 検査に加え、コンパイル時に独立した Lean カーネルを実行する。ホストが同じ契約を守るという前提で、共有メモリのデータ競合を排除する。

```wat
(module
  (%race-free)
  (import "env" "memory" (memory $shared 1 1 shared))
  (%atomic.cell $counter $shared i32 0)
  (func $add (export "add") (param $n i32) (result i32)
    (%effect reads writes synchronizes)
    (i32.atomic.rmw.add (%atomic.at $counter) $n)))
```

実行できる [fixture](../tests/component-fixtures/atomic-cells.wlt) と [Worker の使用例](../runtime/atomic-cells.test.mjs) を同梱する。

ここではatomicセルだけを使うversion 1の契約を説明する。[readonly領域のversion 2](readonly-regions.md)は、公開後不変の共有payloadとWorkerごとの私有memoryを追加する。スケジューリングと取消しには[汎用Workerホスト](worker-host.md)を使える。

## 検査する規則

- 共有元のメモリは最大1つ。`shared`、初期ページ数と最大ページ数が同じで、raw memory export がないことを要求する。メモリなしの計算・fence も受理する。
- `(%atomic.cell $name $memory i32/i64 byte-offset)` は固定セルを宣言する。初期メモリ内の自然アライメント、セルの重複、宣言数256以下を検査する。宣言は初期値を書き込まない。
- `(%atomic.at $name)` は atomic のアドレス位置にだけ使える。整数や参照として取り出すこと、加減算・cast・関数引数への受け渡しはできない。メモリ指定を省くとセルのメモリを使う。明示指定する場合も一致を要求する。
- full-width の i32 / i64 load・store・RMW・cmpxchg、同じ型の wait、notify、fence を扱う。狭幅アクセス、余分な offset、raw アドレス、size/grow は拒否する。notify はセルの待ち行列を操作し、payload の書込みとしては扱わない。
- 未使用の定義を含めて検査する。未知のホスト関数 import、数値以外の ABI 引数・結果を拒否する。unit の結果は許す。GC データはエージェント内に留め、従来の move / 消費規則を維持する。

`%atomic.cell` と `%atomic.at` は `%race-free` なしでも静的な型付き糖衣として使えるが、その場合に証明済みとは扱わない。通常の WAT と、任意のアドレスを使う既存 atomics の経路も維持する。

ファイルからは `(%export (cell $counter))` と `(%use "./state.wlt" (cell $counter $alias))` でセルを選べる。別名は同じセルを指す。`%race-free` はエントリーモジュールに置き、取り込む全定義へ適用する。

## ホスト契約

Core＋JSPI の生成 api は、共有メモリ import に対して明示的な契約指定を要求する。

```js
const memory = new WebAssembly.Memory({ initial: 1, maximum: 1, shared: true });
const { api } = await instantiate({
  imports: { env: { memory } },
  atomicHostContract: 'wlt-atomic-cells-v1',
});
Atomics.add(new Int32Array(memory.buffer), 0, 1);
api.add(2);
```

`manifest.json` の `raceFree.cells` が `{offset, width}` のレイアウトを示す。ホストと、同じメモリを使うすべての Worker / Wasm インスタンスは、次を守る。

1. 共有開始後の payload アクセスには、宣言されたセルの範囲と幅に一致する Atomics / Wasm atomics だけを使う。
2. 通常の TypedArray 書込み、異なる幅による重ね合わせ、別モジュールの data segment による再初期化を行わない。
3. wait / notify は宣言したセルのアドレスを使う。終了は有限 timeout や明示的な notify で制御する。

検証済み Core 成果物は `wlt-core-js-v3` を使う。値の codec は v2 と同じで、v3 は契約 descriptor とホストの明示指定を必須にする。古いランタイムは未対応 ABI として拒否するため、契約確認を読み飛ばして実行しない。通常の成果物は引き続き v2 を使う。

共有前の初期化は任意の初期状態としてモデルの外に置く。メモリの共有属性・limits は Wasm エンジンが照合する。契約指定がなければ api はインスタンス化前に拒否する。

**契約の指定はホストコードの検証ではない。** ホストが指定だけ行って規約を破れば証明は適用できない。生の Core Wasm を直接 instantiate する場合も同じ前提が必要。私有メモリでは外部ホストの契約指定は不要で、Core / Component の両方で使える。

## Lean が証明する範囲

[AtomicCells.lean](../proof/AtomicCells.lean) は、コンパイラが抽出するセルの配置とアクセス命令の一覧を独立に検査する。opcode の whitelist も Lean 側に持ち、非 atomic 命令・異なる幅・配置外のアクセスを拒否する。

検査に通った契約について、任意のエージェント数と任意の長さの実行列を対象に、以下を証明する。

| 定理 | 保証 |
| --- | --- |
| `race_free` | 競合するエージェント間アクセスに非 atomic なものが混じらない |
| `uniform_cells` | 重なる payload アクセスは開始位置・幅が一致する |
| `in_bounds` | 各アクセスは4 / 8バイト、自然アライメント、宣言したメモリの範囲内 |

モデルは、検査済みの任意の命令位置を任意のエージェントが何度でも実行できるようにする。制御フローを過大近似し、実行回数の有限展開は使わない。ホストのイベントも同じセル契約に従う。notify / fence / スレッド内 GC 操作は payload trace の stuttering step とする。

ソースのパース・名前解決・IR 抽出・最適化・DCE・Wasm 出力・エンジン・ABI 実装と、ホストの契約順守は TCB / 仮定として残る。生成 Wasm 全体の意味保存や、任意の JavaScript を形式検証したものではない。

データ競合がなくても、`atomic.load → 加算 → atomic.store` の手順は更新を失い得る。Node の検査では2 Worker を両方 load の直後で止め、両方が1を書いて最終値が1になることを再現する。`rmw.add` なら同じ実行で加算は失われない。線形化可能性・deadlock-free・公平性・排他領域 `Owned<R>` の分割／移動はこの版の保証に含めない。

## コマンドと証拠

```sh
cargo run --locked -- race-contracts tests/component-fixtures/atomic-cells.wlt
cargo run --locked -- verify tests/component-fixtures/atomic-cells.wlt
just test-atomic-cells
```

`check` と `race-contracts` は Rust の構造検査・契約抽出だけを行う。`build` / `wat` / `core-js` / `component` / `verify` は、対象コードの Lean 検査にも成功する必要がある。ブラウザ内コンパイラは Lean を実行できないため、このモードを `E_HOST_UNAVAILABLE` で拒否する。ネイティブで検証して生成した成果物はブラウザで実行できる。

設計判断と証拠:

| 項目 | 内容 |
| --- | --- |
| 契約の出所 | 固定幅の atomic セル限定と、ホスト契約を前提とするユーザーの選択 |
| 実装 | セル参照を定数アドレスへ下げ、全関数の memory 命令を検査。検証モードでは opaque fallback を禁止 |
| 機械検査 | Lean の trace 不変条件と3定理。Rust / Node / ブラウザで受理・拒否と実行を照合 |
| 拒否する witness | 部分的に重なる i64 / i32 セル、i32.store、異なる幅、契約外 offset。改ざんした JSON もカーネルが拒否 |
| 保持する witness | 全操作が atomic な load/store プロトコルの更新喪失。データ競合の証明と論理的な競合を区別 |
| ホスト境界 | ホストが非 atomic 書込みを行う反例をモデルに保持。ホスト契約を外すと保証は成立しない |
