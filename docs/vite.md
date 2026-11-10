# ViteからWLTをESMとして使う

`.wlt`を既存のCore＋JSPI境界へコンパイルし、JavaScriptのnamed exportとして読み込む。
Vite 8.3以降、Node.js 24以降と、同じ版の`wlt` CLIを使う。
ブラウザにはWasm GCが必要で、非同期関数にはJSPIも必要になる。

リポジトリ内では次のサンプルを動かせる。

```sh
pnpm install --frozen-lockfile
just vite          # 開発サーバー
just build-vite    # 本番ビルドとTypeScript検査
```

[サンプル](../examples/vite/)はstring、bytes、i64、Future、明示的なインスタンス生成を扱う。
別プロジェクトからローカルのチェックアウトを使う場合は、同じ版のCLIとプラグインを用意する。

```sh
cargo install --path ../wlt-lang --locked --bin wlt
pnpm add -D vite @mizchi/wlt-runtime@file:../wlt-lang
```

```ts
// vite.config.ts
import {defineConfig} from 'vite';
import wlt from '@mizchi/wlt-runtime/vite';

export default defineConfig({plugins:[wlt({dts:true})]});
```

```wat
;; math.wlt
(module
  (func $add (export "add") (param $a i32) (param $b i32)
    (result i32) (%effect pure) (+ $a $b)))
```

```ts
import {add} from './math.wlt';
console.log(add(20, 22)); // 42
```

通常のimportはtop-level awaitで初期化する非同期ESMになる。同じESM IDではインスタンスを共有し、
同期関数は呼出し時に同期の値を返す。動的`import()`でも読み込める。
同じインスタンスの非同期処理が実行中なら、同期関数は`start()`でキューへ入れるか別インスタンスから呼ぶ。
export名は既存のJS境界と同じ変換を使うため、WLTの`"add-one"`はJSの`addOne`になる。
WLT自身が`"default"`をexportした場合はその関数がdefault exportになる。

この方式は[ViteのWasm ESM Integration](https://vite.dev/guide/features#esm-integration)と同じ、
非同期のモジュール初期化とnamed exportを入口にする。WLTの値には既存のcodec・所有権・取消しの境界を使う。
Wasm GC参照をJSへ直接公開する用途や、`import source`は扱わない。

## 明示的な初期化とホスト

`?init`のdefault exportは、既存の`instantiate(options)`を呼ぶファクトリになる。
呼ぶたびに新しいインスタンスを作り、`{api, start}`を返す。コンパイル済みWasm Moduleは再利用する。

```ts
import init, {TaskCancelledError} from './math.wlt?init';
import type {WltExports} from './math.wlt';

const {api, start} = await init<WltExports>();
console.log(api.add(20, 22));
const task = start('add', [20, 22]);
console.log(await task.result);
```

カスタムhost、共有Memory、eventLoop、task通知などのoptionsは[既存ランタイム](runtime.md)と共通。
`?init`ではホストを自動リンクせず、`init({imports:{host:provider}})`のように明示的に渡す。
共有Memoryの証明を持つプログラムは`?init`と`atomicHostContract`／`readonlyHostContract`を使う。

通常のESM importでは、WLTのhost importのmodule名をJSのspecifierとして解決する。
`"./host.mjs"`ならWLTファイルの隣を読む。カスタムimportには従来どおり型・effectを定めるbinding policyが必要になる。
namespaceからproviderへの対応は設定でも指定できる。

```ts
wlt({
  bindings:'./bindings.json',
  imports:{host:'./src/host.ts'},
});
```

設定内の相対パスはViteのrootが基準。JS providerはnamed exportのオブジェクトとして接続する。
値はインスタンス生成時に取り込み、既存インスタンスのimportを後から差し替えない。
組み込みevent-loopは既存の既定実装を使い、その他の`wlt:` capabilityは明示的なmappingか`?init`を必要とする。

## 型定義

TS7のContent Mapperを使う場合は、宣言ファイルの生成なしで検査できる。
設定と対応するホストの版は[Content Mapperのガイド](content-mapper.md)を参照。

`dts:true`は検査済みの公開ABIから`math.d.wlt.ts`を生成する。
`allowArbitraryExtensions:true`と`@mizchi/wlt-runtime/vite/client`の型を有効にする。

```json
{
  "compilerOptions": {
    "strict": true,
    "noEmit": true,
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "target": "ESNext",
    "allowArbitraryExtensions": true,
    "types": ["@mizchi/wlt-runtime/vite/client"]
  }
}
```

TypeScriptは[Vite公式の任意拡張子の宣言方式](https://vite.dev/guide/features#esm-integration)でnamed exportを解決する。
生成はモジュールの初回コンパイル時なので、`vite build`の後に`tsc --noEmit`を実行するか、生成物を管理する。
サンプルには対応する生成済み宣言を含めている。手書きの宣言は上書きせず、検査失敗時には自身が生成した古い宣言を消す。
リポジトリ内のサンプルは自己参照パッケージの型探索を補うため相対`types`／`paths`を使っている。

i64はbigint、bytesはUint8Array、数値listの戻り値は対応するTypedArrayになる。
Option、Result、record、variant、enum、flags、tupleも既存のJS表現へ型付けする。
非同期関数はPromiseを返す。同期関数のFuture戻り値は`WltFuture<T>`で、PromiseLikeと明示解放を提供する。
Streamは`WltStream<T>`で、`next()`・`readMany()`・`return()`を提供する。
同期Futureを`start()`経由で受け取るresultは、ランタイム固有のthenable同化抑制をTypeScriptで表し切れないためunknownとする。
通常の関数呼出しではFutureハンドルの型を保持する。

## ビルドと開発時の更新

本番のブラウザビルドでは2つのWasmをViteのassetとして出力し、設定したbaseに合わせて読み込む。
開発時とSSRではバイナリを生成JSへ埋め込み、ファイル配信やブラウザのNode互換機能に依存せずロードする。
SSRでも同じCore／JSPIのランタイム条件が適用される。

コンパイラが実際に読んだ`%use`／`%include`／typed Wasmとbinding policyを監視する。
WLTの変更ではページ全体を再読み込みし、型やGCレイアウトが変わった古いインスタンスを差し替えない。
コンパイル失敗はViteのerror overlayに元ファイルの位置を表示する。

コンパイラはシェルを介さず実行する。必要なら`compiler:'./bin/wlt'`や
`compiler:['cargo','run','--locked','--manifest-path','/path/to/Cargo.toml','--']`で指定できる。
既定は環境変数`WLT`、なければPATH上の`wlt`。タイムアウトは`timeoutMs`（既定60000）。
ビルド用の一時ファイルは成功・失敗・終了のいずれでも回収する。

静的な`%use`のDCEはコンパイラの既存規則に従う。JSで未使用のbindingが消えても、公開したWasm exportはビルドの根として残る。
Core＋JSPIの公開ABIの制約も引き継ぐ。`?raw`と`?url`はViteが元のWLTソースに対して処理する。

検査は`just test-vite`。SSR、元ファイルの診断、宣言の型検査、別インスタンス、ABI値に加え、
Chromiumで開発／本番のサブパス配信と依存ファイル変更・エラー復帰を確認する。
