# Walz + Vite

English | [日本語](README.ja.md)

```sh
# From the repository root, with Node.js 24+ and pnpm 12 installed:
pnpm install --frozen-lockfile
just vite
```

The page imports named functions from `program.walz` and creates separate instances through `program.walz?init`.
It demonstrates strings, bytes, BigInt, Promises, and Wasm traps. Edit `helper.walz` to trigger a full page reload.

`just build-vite` creates `target/vite-example/` and checks the generated TypeScript declarations.
The checked-in `program.d.walz.ts` is generated from the public ABI and is refreshed by the plugin.

See [the integration guide](../../docs/vite.md) for installation in another project and explicit host imports.
