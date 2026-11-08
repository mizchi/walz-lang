# Alternating comparison

Same inputs and serialized indexes, captured Wasm, local execution with identical input/output copies. Nine rotated samples; milliseconds per batch.

| Workload | Q | Before | After | Before / after |
|---|---:|---:|---:|---:|
| rank | 32768 | 0.250 | 0.276 | 0.907 |
| select | 4096 | 0.122 | 0.114 | 1.068 |
| quantile | 1024 | 0.110 | 0.094 | 1.165 |
| quantile | 32768 | 6.240 | 5.895 | 1.058 |
| range | 32768 | 13.812 | 10.610 | 1.302 |
