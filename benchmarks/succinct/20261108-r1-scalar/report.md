# Succinct index benchmark

Node v24.21.0, V8 13.6.233.17-node.53, Apple M5. Nine rotated samples; milliseconds per batch.

Construction/worker startup are excluded from steady-state rows and recorded separately in report.json. WLT rows include query copy, execution and output copy; worker rows also include messages and result merging. jsimd is its captured public API. Range queries use its scalar API loop. WLT quantile outputs i32; jsimd outputs u8.

| Workload | N | Q | Indexed JS | jsimd | WLT local | WLT 1 worker | WLT 2 | WLT 4 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| rank | 262144 | 32768 | 0.551 | 0.343 | 0.239 | 0.321 | 0.202 | 0.147 |
| select | 262144 | 4096 | 0.124 | 5.157 | 0.109 | 0.161 | 0.096 | 0.072 |
| quantile | 262144 | 1024 | 0.281 | 0.099 | 0.115 | 0.144 | 0.096 | 0.080 |
| quantile | 262144 | 32768 | 11.162 | 6.199 | 6.418 | 6.516 | 3.340 | 1.734 |
| range | 262144 | 32768 | 19.860 | 13.776 | 14.386 | 14.676 | 7.471 | 4.100 |

No comparison attributes the indexed-select algorithm improvement over linear jsimd select to WLT alone. The indexed JS baseline uses the same directory, tail mask and upper_bound strategy.

Load/compile: WLT 2.31 ms; jsimd 34.29 ms. These are single observations, not steady-state medians.
