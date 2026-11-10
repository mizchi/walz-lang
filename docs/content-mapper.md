# TS7 Content Mapper

WLTの公開ABIを仮想TypeScriptへ変換し、`.d.wlt.ts`を生成せずに`.wlt`をimportして型チェックする。
Viteプラグインと同じABI型生成を使う。WLTの実行コードは[Viteプラグイン](vite.md)が生成し、
Content Mapperは検査と型情報だけを提供する。
TypeScriptからのJavaScript・declaration出力はサポート対象外とし、`noEmit`で使う。

[TypeScript upstreamのContent Mapper](https://github.com/microsoft/TypeScript/blob/b7917e5e4c278f132ed19f777d968b6b1180fc24/tsc/internal/contentmapper/hostimpl.go)
に対応する。検証対象は`content-mapper/upstream.json`に固定したTypeScript 7.1.0-dev。
Content Mapperを備えたnative TypeScriptが必要で、既存の`typescript@7.0.2`による宣言ファイルの検査も残している。
[Vizeの説明](https://vizejs.dev/ja/guide/content-mapper/index.html)と同じ`contentMappers`設定を使う。

## 設定

Node.js 24以降、同じ版の`wlt` CLIと`@mizchi/wlt-runtime`を用意する。
ローカルチェックアウトのインストール方法は[Viteのガイド](vite.md)を参照。

```json
{
  "compilerOptions": {
    "target": "ESNext",
    "module": "preserve",
    "strict": true,
    "noEmit": true
  },
  "contentMappers": [
    {
      "package": "@mizchi/wlt-runtime",
      "extensions": [".wlt"],
      "options": {
        "compiler": "wlt"
      }
    }
  ],
  "include": ["src"]
}
```

```sh
tsgo --runExternalCode --noEmit -p tsconfig.json
```

ホストのバイナリ名が`tsc`なら、その実行ファイルに同じ引数を渡す。
`--runExternalCode`はTypeScriptが外部mapperを起動するための明示指定。
Viteの`dts`設定は`false`のままでよい。i64はbigint、bytesはUint8Array、FutureやStreamなどの型は既存のJS境界と共通。
`?init`には引き続き`@mizchi/wlt-runtime/vite/client`の型と、通常のimportから得る`WltExports`を使える。

## オプションと依存ファイル

| オプション | 既定 | 意味 |
| --- | --- | --- |
| `compiler` | 環境変数`WLT`、または`wlt` | 実行ファイル名、または実行ファイルと引数の配列 |
| `timeoutMs` | `60000` | 各検査のタイムアウト |
| `bindings` | なし | ホストimportのbinding policy |
| `root` | tsconfigのディレクトリ | WLT依存ファイルの監視範囲 |
| `watchFiles` | `[]` | 範囲外の依存ファイルを追加する絶対・相対パス |

相対パスはtsconfigのディレクトリが基準。compilerはシェルを介さず起動し、プロジェクトごとにオプションを保持する。
オプションが不正ならtsconfig上に診断を返し、検査を進めない。

`root`以下の`.wlt`・`.wit`・`.wasm`と、binding policy・`watchFiles`を追加の変換入力としてハッシュ化する。
TypeScriptがファイル単位の変換結果を再利用しても、WLTの依存ファイルを変更した場合は再検査が必要なため、
これらを`dynamicConfig`の監視対象にも登録する。`node_modules`・`target`・`.git`・`.cache`・`dist`とsymlinkは再帰探索しない。
範囲外の依存を検出したら`watchFiles`への追加を求め、古い型を返さない。大きいmonorepoでは`root`を対象ソースのディレクトリへ絞る。

## エディターと位置情報

TS7のホストが渡す現在のWLTバッファを`wlt esm-metadata --stdin`で検査する。
ディスクのファイルを上書きせず、Wasm・JS・型宣言も出力しない。
型・effect・所有権・race・lawと公開ABIはCore＋JSPIの検査経路を通す。
`%use`などの依存は保存済みファイルを読む。複数WLTファイルの未保存バッファを一括で扱う機能は含めない。

JSON-RPCの`initialize`・`openProject`・`transform`・`closeProject`とUTF-8の位置に対応する。
TS側のホバー・シグネチャと、元のWLT exportへの定義ジャンプを提供する。
export名にはcamelCase変換があるため、変換位置を使ったリネーム・編集は許可しない。
マクロが生成したexportや別ファイルのexportなど、正確な位置を対応付けられない場合はマッピングを作らない。
WLT内部の補完・リネーム・所有権情報は既存の`wlt lsp`が担当する。

WLTの検査エラーは元バッファの位置へ返す。別の依存ファイルのエラーはそのファイル名・行・列をメッセージに含め、
現在のバッファの位置として扱わない。失敗時には空の仮想モジュールを返し、前回の型を再利用しない。

LSPクライアントは信頼されたworkspaceで`initializationOptions.runExternalCode:true`を渡す必要がある。
WLT用VS Code拡張によるTSホストへの自動登録は含めない。

## 検証

```sh
just test-content-mapper        # Rustの検査APIとJSON-RPC、mapperの検査
just test-content-mapper-tsgo   # 固定したupstreamをビルドし、CLI・LSPで検証
```

後者はgitとGoが必要。Goのtoolchainはupstreamのgo.modに従う。
生成バイナリは`target/content-mapper-tools/tsgo`に置く。

```sh
WLT="$PWD/target/debug/wlt" target/content-mapper-tools/tsgo \
  --runExternalCode --noEmit -p examples/content-mapper
```

CLIでは宣言ファイルなしのimport、型不一致、WLT診断と復帰を検査する。
LSPではTSのホバー、WLT exportへの定義ジャンプ、未保存バッファによる型の更新を検査する。
watchでは保存済みの依存WLTを変更し、親モジュールの型エラー検出と復帰を確認する。
リポジトリ内のサンプルは未インストールの自己パッケージを参照するため`package:"../.."`を使う。
