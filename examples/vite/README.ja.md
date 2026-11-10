# Walz + Vite

[English](README.md) | 日本語

```sh
# From the repository root, with Node.js 24+ and pnpm 12 installed:
pnpm install --frozen-lockfile
just vite
```

ページは `program.walz` から名前を指定して関数を import し、`program.walz?init` を通じて個別のインスタンスを作成します。
文字列、bytes、BigInt、Promise、Wasm trap の例を含みます。`helper.walz` を編集するとページ全体を再読み込みします。

`just build-vite` は `target/vite-example/` を生成し、生成された TypeScript 宣言を検査します。
リポジトリに保存されている `program.d.walz.ts` は公開 ABI から生成され、プラグインによって更新されます。

別のプロジェクトへの導入方法と明示的なホスト import は、[連携ガイド](../../docs/vite.md)を参照してください。
