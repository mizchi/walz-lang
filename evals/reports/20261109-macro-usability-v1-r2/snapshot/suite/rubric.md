# Macro usability rubric v1

Score each axis 0–4: 0 cannot complete; 1 needs implementation knowledge or a substantial workaround; 2 completes with recurring friction; 3 works after a small documentation lookup; 4 locally understandable and predictable. Supply concrete code, command, diagnostic or protocol evidence for every score.

| Axis | Questions |
| --- | --- |
| Consumer readability | Can the validation order and failure path be followed at the call site? Does it improve on ordinary functions as well as inline code? |
| Macro authoring | Can the abstraction be defined with the documented grammar? How much helper syntax and implementation knowledge is required? |
| Composition and changes | Can field additions and policy changes stay local? Can binding macros be delegated or repeated? |
| Static guarantees | Are types/effects/ownership checked after expansion? Are evaluation order and repeated effects explicit? |
| Diagnostics and LSP | Do errors point to the actual input or definition? Do hover, completion and definitions remain useful in broken/current revisions? |
| Cost and scale | Does a modest app compile, and how do elapsed time and limits change with call-site count? Are runtime and code-size costs material? |

Do not average away a weak axis. Separate required correctness checks, desired-task completion, authoring attempts, measured auxiliary metrics, and evaluator judgment. Do not rank the language against unrelated languages from this single experiment.
