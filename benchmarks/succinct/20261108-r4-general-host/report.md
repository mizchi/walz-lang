# Succinct index benchmark

Node v24.21.0, V8 13.6.233.17-node.53, Apple M5. Nine rotated samples; milliseconds per batch.

Construction/worker startup are excluded from steady-state rows and recorded separately in report.json. WLT rows include query copy, execution and output copy; worker rows also include messages and result merging. jsimd is its captured public API. Range queries use its scalar API loop. WLT quantile outputs i32; jsimd outputs u8.

| Workload | N | Q | Indexed JS | jsimd | WLT local | WLT 1 worker | WLT 2 | WLT 4 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| rank | 4096 | 1 | 0.0001 | 0.0007 | 0.0004 | 0.0176 | 0.0217 | 0.0372 |
| rank | 262144 | 32768 | 0.5436 | 0.3387 | 0.2559 | 0.3380 | 0.2163 | 0.1458 |
| rank (sparse) | 1048576 | 32768 | 0.5485 | 0.3262 | 0.2451 | 0.3085 | 0.2063 | 0.1445 |
| select | 262144 | 4096 | 0.1126 | 5.0487 | 0.1222 | 0.1544 | 0.0956 | 0.0811 |
| quantile | 262144 | 1024 | 0.2619 | 0.0962 | 0.0916 | 0.1301 | 0.0845 | 0.0626 |
| quantile | 262144 | 32768 | 11.0955 | 6.1573 | 6.1370 | 6.2624 | 3.2222 | 1.6920 |
| quantile | 1048576 | 32768 | 11.5602 | 6.4096 | 6.5250 | 6.5479 | 3.4130 | 1.8162 |
| quantile (low-alphabet) | 262144 | 32768 | 8.4168 | 4.1111 | 4.2111 | 4.2882 | 2.2246 | 1.1638 |
| range | 262144 | 32768 | 18.8998 | 13.4896 | 11.1073 | 11.1843 | 5.6880 | 2.9234 |

No comparison attributes the indexed-select algorithm improvement over linear jsimd select to WLT alone. The indexed JS baseline uses the same directory, tail mask and upper_bound strategy.

Load/compile: WLT 2.14 ms; jsimd 36.67 ms. These are single observations, not steady-state medians.
