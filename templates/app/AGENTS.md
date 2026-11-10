# Walz application

Use the local [Walz skill](.agents/skills/walz/SKILL.md) when reading or changing `.walz` programs.
The [cheat sheet](CHEATSHEET.md) and [CLI examples](.agents/skills/walz/references/cli.md) are bundled references.

- Put reusable code in `src/` with explicit `@export` declarations. Keep Wasm exports in the root entry and tests/benchmarks in their own files.
- Check an affected entry with `walz check src/main.walz` and run `walz test` after changes.
- Preserve type, effect, and ownership contracts. Host capabilities are explicit.
- `walz run` and the default tests require Wasmtime with Wasm GC; benchmarks and Core＋JSPI require Node.js 24+.
