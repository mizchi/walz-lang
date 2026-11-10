# WLT + Vite

```sh
# From the repository root, with Node.js 24+ and pnpm 12 installed:
pnpm install --frozen-lockfile
just vite
```

The page imports named functions from `program.wlt` and creates separate instances through `program.wlt?init`.
It demonstrates strings, bytes, BigInt, Promises, and Wasm traps. Edit `helper.wlt` to trigger a full page reload.

`just build-vite` creates `target/vite-example/` and checks the generated TypeScript declarations.
The checked-in `program.d.wlt.ts` is generated from the public ABI and is refreshed by the plugin.

See [the integration guide](../../docs/vite.md) for installation in another project and explicit host imports.
