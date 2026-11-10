# TS7 Content Mapper

Converts Walz's public ABI to virtual TypeScript so that `.walz` imports can be type-checked without generating `.d.walz.ts` files.
It uses the same ABI type generation as the Vite plugin. The [Vite plugin](vite.md) generates Walz's executable code;
the Content Mapper provides checking and type information only.
JavaScript and declaration output from TypeScript are unsupported; use `noEmit`.

Supports [TypeScript upstream's Content Mapper](https://github.com/microsoft/TypeScript/blob/b7917e5e4c278f132ed19f777d968b6b1180fc24/tsc/internal/contentmapper/hostimpl.go).
Validation targets TypeScript 7.1.0-dev pinned in `content-mapper/upstream.json`.
Native TypeScript with Content Mapper support is required. Declaration-file checks using the existing `typescript@7.0.2` are also retained.
It uses the same `contentMappers` configuration described in [Vize's guide](https://vizejs.dev/ja/guide/content-mapper/index.html).

<a id="設定"></a>

## Setup

Install Node.js 24 or later and matching versions of the `walz` CLI and `@mizchi/walz-runtime`.
For installation from a local checkout, see the [Vite guide](vite.md).

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
      "package": "@mizchi/walz-runtime",
      "extensions": [".walz"],
      "options": {
        "compiler": "walz"
      }
    }
  ],
  "include": ["src"]
}
```

```sh
tsgo --runExternalCode --noEmit -p tsconfig.json
```

If the host binary is named `tsc`, pass the same arguments to that executable.
`--runExternalCode` explicitly permits TypeScript to start an external mapper.
Vite's `dts` option can remain `false`. i64 maps to bigint, bytes to Uint8Array, and Future, Stream, and other types share the existing JS boundary definitions.
For `?init`, continue using the types from `@mizchi/walz-runtime/vite/client` and `WalzExports` from a normal import.

<a id="オプションと依存ファイル"></a>

## Options and dependencies

| Option | Default | Meaning |
| --- | --- | --- |
| `compiler` | Environment variable `WALZ`, or `walz` | Executable name, or an array of the executable and its arguments |
| `timeoutMs` | `60000` | Timeout for each check |
| `bindings` | none | Binding policy for host imports |
| `root` | tsconfig directory | Watch scope for Walz dependencies |
| `watchFiles` | `[]` | Absolute or relative paths adding dependencies outside the scope |

Relative paths are resolved from the tsconfig directory. The compiler is started without a shell, and options are stored per project.
Invalid options produce diagnostics on tsconfig and prevent checking from proceeding.

The mapper hashes `.walz`, `.wit`, and `.wasm` files under `root`, the binding policy, and `watchFiles` as additional transformation inputs.
Because Walz dependencies require rechecking even when TypeScript reuses a per-file transformation result,
these files are also registered as `dynamicConfig` watch targets. Recursive traversal excludes `node_modules`, `target`, `.git`, `.cache`, `dist`, and symlinks.
Dependencies outside the scope require an addition to `watchFiles`; stale types are not returned. In large monorepos, narrow `root` to the relevant source directory.

<a id="エディターと位置情報"></a>

## Editors and source positions

The current Walz buffer supplied by the TS7 host is checked with `walz esm-metadata --stdin`.
It does not overwrite the disk file or emit Wasm, JS, or type declarations.
Types, effects, ownership, races, laws, and the public ABI pass through the Core＋JSPI checking path.
Dependencies such as `@use` are read from saved files. Handling unsaved buffers across multiple Walz files together is outside the scope.

Supports JSON-RPC `initialize`, `openProject`, `transform`, and `closeProject`, and UTF-8 positions.
Provides TS-side hover and signatures, and go-to-definition to the original Walz export.
Because export names undergo camelCase conversion, renaming and editing through mapped positions are not allowed.
No mapping is created when positions cannot be matched precisely, such as macro-generated exports or exports from another file.
The existing `walz lsp` handles completion, renaming, and ownership information inside Walz.

Walz checking errors are returned at positions in the original buffer. Errors in other dependency files include that file's name, line, and column in the message,
and are not treated as positions in the current buffer. On failure, an empty virtual module is returned rather than reusing the previous types.

LSP clients must pass `initializationOptions.runExternalCode:true` in trusted workspaces.
Automatic registration with the TS host by the Walz VS Code extension is outside the scope.

<a id="検証"></a>

## Validation

```sh
just test-content-mapper        # Rustの検査APIとJSON-RPC、mapperの検査
just test-content-mapper-tsgo   # 固定したupstreamをビルドし、CLI・LSPで検証
```

The second task requires git and Go. The Go toolchain follows upstream's go.mod.
The generated binary is placed at `target/content-mapper-tools/tsgo`.

```sh
WALZ="$PWD/target/debug/walz" target/content-mapper-tools/tsgo \
  --runExternalCode --noEmit -p examples/content-mapper
```

CLI checks cover imports without declaration files, type mismatches, Walz diagnostics, and recovery.
LSP checks cover TS hover, go-to-definition to Walz exports, and type updates from unsaved buffers.
Watch checks modify a saved Walz dependency and verify detection of type errors in the parent module and recovery.
The repository sample uses `package:"../.."` because it references its own package before installation.
