# Succinct index benchmark

Node v24.21.0, V8 13.6.233.17-node.53, Apple M5. Nine rotated samples; milliseconds per batch.

Construction/worker startup are excluded from steady-state rows and recorded separately in report.json. WLT rows include query copy, execution and output copy; worker rows also include messages and result merging. jsimd is its captured public API. Range queries use its scalar API loop. WLT quantile outputs i32; jsimd outputs u8.

| Workload | N | Q | Indexed JS | jsimd | WLT local | WLT 1 worker | WLT 2 | WLT 4 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| rank | 262144 | 32768 | 0.542 | 0.291 | 0.318 | 0.381 | 0.230 | 0.161 |
| select | 262144 | 4096 | 0.163 | 5.312 | 0.144 | 0.176 | 0.103 | 0.086 |
| quantile | 262144 | 1024 | 0.254 | 0.102 | 0.103 | 0.133 | 0.081 | 0.063 |
| quantile | 262144 | 32768 | 11.156 | 6.139 | 6.132 | 6.229 | 3.191 | 1.692 |
| range | 262144 | 32768 | 19.107 | 13.229 | 10.949 | 11.095 | 5.677 | 2.950 |

No comparison attributes the indexed-select algorithm improvement over linear jsimd select to WLT alone. The indexed JS baseline uses the same directory, tail mask and upper_bound strategy.

Load/compile: WLT 2.60 ms; jsimd 31.31 ms. These are single observations, not steady-state medians.
