# SIMD kernel measurements

v24.21.0 / Apple M5; 9 rounds. Median microseconds/call.

Resident excludes construction and packing; wltPackAndSimd includes packing from existing scalar GC arrays. jsimdKernel is the captured raw linear-memory kernel, without its public JS wrapper.

| workload | n | js | wltScalar | wltSimd | jsimdKernel | wltPackAndSimd |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| i32-sum | 16 | 0.012 | 0.023 | 0.021 | 0.013 | 0.037 |
| f32-dot | 16 | 0.012 | 0.026 | 0.022 | 0.006 | 0.064 |
| bitmap-intersection | 16 | 0.018 | 0.029 | 0.023 | 0.009 | 0.069 |
| bytes-index-miss | 16 | 0.013 | 0.017 | 0.013 | 0.005 | 0.033 |
| bytes-index-late | 16 | 0.012 | 0.018 | 0.014 | 0.008 | 0.036 |
| bytes-index-early | 16 | 0.011 | 0.013 | 0.014 | 0.008 | 0.037 |
| i32-sum | 1024 | 0.530 | 0.370 | 0.117 | 0.100 | 0.478 |
| f32-dot | 1024 | 0.632 | 0.636 | 0.162 | 0.114 | 0.786 |
| bitmap-intersection | 1024 | 0.939 | 0.469 | 0.179 | 0.346 | 0.863 |
| bytes-index-miss | 1024 | 0.260 | 0.329 | 0.046 | 0.035 | 0.387 |
| bytes-index-late | 1024 | 0.260 | 0.331 | 0.045 | 0.036 | 0.390 |
| bytes-index-early | 1024 | 0.011 | 0.013 | 0.014 | 0.010 | 0.356 |
| i32-sum | 4096 | 2.148 | 1.415 | 0.476 | 0.460 | 1.767 |
| f32-dot | 4096 | 2.576 | 2.554 | 0.638 | 0.563 | 3.007 |
| bitmap-intersection | 4096 | 3.983 | 1.800 | 0.731 | 1.411 | 3.332 |
| bytes-index-miss | 4096 | 0.955 | 1.258 | 0.142 | 0.120 | 1.430 |
| bytes-index-late | 4096 | 0.975 | 1.269 | 0.144 | 0.121 | 1.471 |
| bytes-index-early | 4096 | 0.012 | 0.014 | 0.015 | 0.010 | 1.337 |
| i32-sum | 16384 | 8.719 | 5.558 | 1.902 | 1.892 | 6.644 |
| f32-dot | 16384 | 10.349 | 10.182 | 2.537 | 2.536 | 11.126 |
| bitmap-intersection | 16384 | 16.000 | 7.196 | 2.878 | 6.001 | 12.927 |
| bytes-index-miss | 16384 | 3.786 | 4.955 | 0.534 | 0.458 | 5.581 |
| bytes-index-late | 16384 | 3.840 | 4.968 | 0.542 | 0.466 | 5.640 |
| bytes-index-early | 16384 | 0.012 | 0.014 | 0.015 | 0.010 | 5.110 |
| i32-sum | 262144 | 142.665 | 86.168 | 30.675 | 30.514 | 174.941 |
| f32-dot | 262144 | 170.189 | 170.559 | 45.295 | 44.715 | 413.266 |
| bitmap-intersection | 262144 | 246.799 | 113.535 | 45.776 | 90.950 | 323.090 |
| bytes-index-miss | 262144 | 60.968 | 79.096 | 8.324 | 7.191 | 119.258 |
| bytes-index-late | 262144 | 60.753 | 78.821 | 8.285 | 7.168 | 114.622 |
| bytes-index-early | 262144 | 0.012 | 0.014 | 0.015 | 0.010 | 105.196 |

Copy-inclusive includes input copies and the kernel. WLT uses its production value adapters, GC conversion and packing; jsimdScratchCopy reuses preallocated linear scratch and excludes its public allocator/wrapper. These are representation/boundary costs, not equal API implementations.

| workload | n | jsimdScratchCopy | coreScalar | coreSimd | componentScalar | componentSimd |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| i32-sum | 16 | 0.021 | 1.190 | 1.612 | 2.969 | 3.016 |
| f32-dot | 16 | 0.020 | 1.947 | 1.872 | 3.874 | 3.164 |
| bitmap-intersection | 16 | 0.024 | 1.857 | 2.120 | 3.030 | 2.992 |
| bytes-index-miss | 16 | 0.015 | 0.891 | 0.655 | 2.846 | 3.871 |
| bytes-index-late | 16 | 0.018 | 0.839 | 0.991 | 2.826 | 4.107 |
| bytes-index-early | 16 | 0.018 | 0.829 | 0.961 | 2.770 | 3.989 |
| i32-sum | 1024 | 0.163 | 35.442 | 35.205 | 3.920 | 4.467 |
| f32-dot | 1024 | 0.242 | 70.943 | 70.772 | 5.773 | 5.820 |
| bitmap-intersection | 1024 | 0.432 | 70.021 | 70.683 | 5.681 | 5.932 |
| bytes-index-miss | 1024 | 0.052 | 1.862 | 1.665 | 3.823 | 4.736 |
| bytes-index-late | 1024 | 0.054 | 2.063 | 1.907 | 4.678 | 3.865 |
| bytes-index-early | 1024 | 0.029 | 1.524 | 2.116 | 3.904 | 3.858 |
| i32-sum | 4096 | 0.632 | 139.951 | 140.656 | 8.482 | 9.102 |
| f32-dot | 4096 | 0.888 | 280.004 | 289.115 | 13.075 | 13.864 |
| bitmap-intersection | 4096 | 1.691 | 274.719 | 282.568 | 12.398 | 14.302 |
| bytes-index-miss | 4096 | 0.165 | 5.088 | 5.329 | 7.258 | 7.694 |
| bytes-index-late | 4096 | 0.166 | 4.869 | 5.263 | 7.036 | 7.405 |
| bytes-index-early | 4096 | 0.056 | 3.735 | 5.233 | 5.803 | 7.334 |
| i32-sum | 16384 | 2.742 | 555.690 | 561.776 | 22.266 | 23.695 |
| f32-dot | 16384 | 4.891 | 1144.792 | 1136.948 | 40.529 | 42.821 |
| bitmap-intersection | 16384 | 7.746 | 1122.318 | 1114.953 | 37.878 | 43.416 |
| bytes-index-miss | 16384 | 0.604 | 16.671 | 17.875 | 19.291 | 20.166 |
| bytes-index-late | 16384 | 0.606 | 16.793 | 17.809 | 19.091 | 20.062 |
| bytes-index-early | 16384 | 0.193 | 11.573 | 17.379 | 13.844 | 19.559 |
| i32-sum | 262144 | 43.447 | 9036.000 | 9007.542 | 419.313 | 494.651 |
| f32-dot | 262144 | 67.471 | 19066.167 | 18645.750 | 799.865 | 984.740 |
| bitmap-intersection | 262144 | 117.305 | 18740.833 | 18132.625 | 780.016 | 917.698 |
| bytes-index-miss | 262144 | 10.550 | 331.754 | 348.829 | 307.798 | 354.794 |
| bytes-index-late | 262144 | 10.540 | 325.591 | 345.422 | 304.697 | 355.396 |
| bytes-index-early | 262144 | 3.151 | 243.995 | 337.292 | 226.737 | 340.466 |
