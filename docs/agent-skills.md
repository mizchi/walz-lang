# Reading and writing Walz with an agent

The repository includes a portable [Walz skill](../.agents/skills/walz/SKILL.md) for application source, source-module boundaries, types, effects, and ownership.
It is distinct from instructions for changing the Rust compiler.

## Included resources

| File | Use |
| --- | --- |
| [SKILL.md](../.agents/skills/walz/SKILL.md) | Reading/writing workflow, non-obvious language rules, and validation |
| [Syntax reference](../.agents/skills/walz/references/syntax.md) | Compact patterns and complete compiler-checked examples |
| [CLI examples](../.agents/skills/walz/references/cli.md) | Scaffold, check, inspect, execute, test, and benchmark commands |
| [English cheat sheet](../CHEATSHEET.md), [Japanese cheat sheet](../CHEATSHEET.ja.md) | Full language lookup |
| [English specification](../SPEC.md), [Japanese specification](../SPEC.ja.md) | Precise semantics and supported boundaries |

The skill folder is self-contained: its essential references do not depend on a compiler checkout.
Its optional `agents/openai.yaml` supplies discovery metadata; ordinary skill selection remains enabled.

## Use it in an application

```sh
walz new app
cd app
walz check src/main.walz
walz test
```

The scaffold includes `.agents/skills/walz`, `AGENTS.md`, and both cheat sheets.
Codex discovers repository skills in `.agents/skills`; see the [official skill documentation](https://learn.chatgpt.com/docs/build-skills).
With a compatible agent, request the `walz` skill explicitly, for example:

```text
Use the walz skill to add a checked Result-returning function and tests to this application.
```

For another skill-compatible tool, copy the complete `walz` folder to its documented skill location.
The generator does not change global agent settings. Copying only SKILL.md would omit its syntax/CLI references.

## Validation and scope

The installed `walz` compiler is the authority for accepted syntax. `walz check --json` and the type-query commands inspect code without executing host functions.
Use `walz test` for behavior and choose the Core＋JSPI target when a test needs external async values or JS ABI conversion.
The skill does not grant permission to publish, push, or invoke unrelated services, and it does not bypass type/effect/ownership checks.

`just test-skill` compiles its complete examples; `just test-new` exercises a generated project through build, run, tests, and benchmark execution.
An independent forward test should use an isolated generated application and report actual compiler/test results plus any missing instructions. Frozen evaluation artifacts remain unchanged.
