# JSON parser tuning comparison

v24.21.0 / Apple M5; 5 rotating rounds; medians. Both binaries run in the same process. Input and benchmark-program hashes match. All raw AST hashes and exact normalized outputs match the baseline. Setup and validation are outside timing; automatic GC inside a batch is included. ABI live allocations are zero after each batch.

## residentParse

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 0.24 | 0.26 | 0.92x |
| records-128 | 33.49 | 37.26 | 0.90x |
| records-512 | 140.60 | 153.92 | 0.91x |
| numbers-4096 | 137.21 | 165.45 | 0.83x |
| wide-object | 93.25 | 105.06 | 0.89x |
| ascii-string | 29.71 | 23.45 | 1.27x |
| unicode-escapes | 19.51 | 19.53 | 1.00x |
| nested-64 | 1.03 | 1.09 | 0.94x |
| whitespace | 26.89 | 26.86 | 1.00x |

## coreParseStatus

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 1.26 | 1.24 | 1.01x |
| records-128 | 50.12 | 50.83 | 0.99x |
| records-512 | 203.69 | 205.04 | 0.99x |
| numbers-4096 | 128.24 | 139.41 | 0.92x |
| wide-object | 95.14 | 102.80 | 0.93x |
| ascii-string | 62.29 | 56.11 | 1.11x |
| unicode-escapes | 22.75 | 21.88 | 1.04x |
| nested-64 | 2.18 | 2.12 | 1.03x |
| whitespace | 67.98 | 67.35 | 1.01x |

## componentParseStatus

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 3.90 | 3.63 | 1.07x |
| records-128 | 57.69 | 57.95 | 1.00x |
| records-512 | 224.71 | 225.70 | 1.00x |
| numbers-4096 | 151.45 | 153.22 | 0.99x |
| wide-object | 121.25 | 132.93 | 0.91x |
| ascii-string | 104.24 | 96.73 | 1.08x |
| unicode-escapes | 31.16 | 28.72 | 1.08x |
| nested-64 | 5.11 | 5.17 | 0.99x |
| whitespace | 127.91 | 119.25 | 1.07x |

## coreNormalize

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 1.85 | 1.83 | 1.01x |
| records-128 | 99.78 | 100.90 | 0.99x |
| records-512 | 425.44 | 425.02 | 1.00x |
| numbers-4096 | 235.39 | 208.04 | 1.13x |
| wide-object | 228.14 | 216.40 | 1.05x |
| ascii-string | 238.69 | 234.84 | 1.02x |
| unicode-escapes | 37.16 | 33.43 | 1.11x |
| nested-64 | 3.69 | 3.70 | 1.00x |
| whitespace | 67.98 | 67.48 | 1.01x |

## componentNormalize

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 4.34 | 4.36 | 1.00x |
| records-128 | 106.22 | 107.48 | 0.99x |
| records-512 | 448.62 | 447.57 | 1.00x |
| numbers-4096 | 254.39 | 232.87 | 1.09x |
| wide-object | 227.89 | 229.54 | 0.99x |
| ascii-string | 278.66 | 272.41 | 1.02x |
| unicode-escapes | 41.12 | 40.78 | 1.01x |
| nested-64 | 6.54 | 6.54 | 1.00x |
| whitespace | 128.42 | 120.43 | 1.07x |

## coreInputLength

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 1.19 | 1.10 | 1.08x |
| records-128 | 18.78 | 18.74 | 1.00x |
| records-512 | 75.04 | 75.25 | 1.00x |
| numbers-4096 | 19.43 | 18.11 | 1.07x |
| wide-object | 17.92 | 18.60 | 0.96x |
| ascii-string | 32.23 | 32.35 | 1.00x |
| unicode-escapes | 3.53 | 3.36 | 1.05x |
| nested-64 | 1.05 | 1.03 | 1.02x |
| whitespace | 39.99 | 40.02 | 1.00x |

## componentInputLength

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 3.29 | 3.15 | 1.05x |
| records-128 | 26.07 | 26.34 | 0.99x |
| records-512 | 96.54 | 97.55 | 0.99x |
| numbers-4096 | 41.76 | 43.98 | 0.95x |
| wide-object | 41.74 | 43.31 | 0.96x |
| ascii-string | 73.54 | 74.49 | 0.99x |
| unicode-escapes | 9.27 | 9.66 | 0.96x |
| nested-64 | 3.48 | 3.46 | 1.00x |
| whitespace | 95.27 | 94.09 | 1.01x |
