# Numeric list ABI comparison

v24.21.0 / Apple M5; 9 interleaved rounds. Median microseconds/call.

Identical guest binaries, workload and measurement code. Timings include input copies, packing, SIMD calculation and ABI cleanup. Array fallback remains validated per element. Component and bytes are controls.

| workload | n | Core before | Core after | speedup | Array before | Array after | Component before | Component after |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| i32-sum | 4096 | 160.223 | 6.353 | 25.22x | 155.590 | 154.214 | 9.506 | 9.581 |
| f32-dot | 4096 | 281.483 | 12.015 | 23.43x | 288.309 | 277.306 | 14.462 | 14.521 |
| bitmap-intersection | 4096 | 315.048 | 12.172 | 25.88x | 313.678 | 309.806 | 14.452 | 14.586 |
| bytes-index-miss | 4096 | 5.396 | 5.383 | 1.00x | — | — | 7.668 | 7.714 |
| i32-sum | 16384 | 612.414 | 22.187 | 27.60x | 592.081 | 594.125 | 25.053 | 25.023 |
| f32-dot | 16384 | 1122.943 | 41.715 | 26.92x | 1110.391 | 1111.521 | 45.632 | 45.199 |
| bitmap-intersection | 16384 | 1221.011 | 42.870 | 28.48x | 1195.432 | 1215.453 | 45.352 | 45.482 |
| bytes-index-miss | 16384 | 18.309 | 18.199 | 1.01x | — | — | 21.181 | 21.384 |
| i32-sum | 262144 | 9582.792 | 496.268 | 19.31x | 9652.625 | 9738.417 | 470.117 | 534.159 |
| f32-dot | 262144 | 19519.917 | 978.159 | 19.96x | 18420.709 | 19469.000 | 1063.776 | 956.068 |
| bitmap-intersection | 262144 | 19507.250 | 935.956 | 20.84x | 19862.958 | 21098.459 | 1063.266 | 904.063 |
| bytes-index-miss | 262144 | 365.310 | 362.915 | 1.01x | — | — | 375.302 | 367.991 |
