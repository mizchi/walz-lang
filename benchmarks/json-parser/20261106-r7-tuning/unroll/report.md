# JSON parser tuning comparison

v24.21.0 / Apple M5; 5 rotating rounds; medians. Both binaries run in the same process. Input and benchmark-program hashes match. All raw AST hashes and exact normalized outputs match the baseline. Setup and validation are outside timing; automatic GC inside a batch is included. ABI live allocations are zero after each batch.

## residentParse

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 0.24 | 0.25 | 0.99x |
| records-128 | 33.29 | 33.39 | 1.00x |
| records-512 | 142.37 | 142.67 | 1.00x |
| numbers-4096 | 133.00 | 133.89 | 0.99x |
| wide-object | 88.36 | 89.71 | 0.98x |
| ascii-string | 30.68 | 27.62 | 1.11x |
| unicode-escapes | 19.68 | 19.87 | 0.99x |
| nested-64 | 1.05 | 1.04 | 1.01x |
| whitespace | 26.86 | 27.60 | 0.97x |

## coreParseStatus

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 1.25 | 1.25 | 1.00x |
| records-128 | 49.69 | 49.82 | 1.00x |
| records-512 | 207.54 | 206.22 | 1.01x |
| numbers-4096 | 124.65 | 123.52 | 1.01x |
| wide-object | 88.47 | 88.62 | 1.00x |
| ascii-string | 63.12 | 61.64 | 1.02x |
| unicode-escapes | 22.26 | 22.15 | 1.00x |
| nested-64 | 2.20 | 2.19 | 1.01x |
| whitespace | 67.64 | 67.55 | 1.00x |

## componentParseStatus

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 3.87 | 3.77 | 1.03x |
| records-128 | 57.93 | 57.87 | 1.00x |
| records-512 | 229.81 | 234.87 | 0.98x |
| numbers-4096 | 157.39 | 147.99 | 1.06x |
| wide-object | 117.18 | 111.39 | 1.05x |
| ascii-string | 115.63 | 98.00 | 1.18x |
| unicode-escapes | 30.09 | 28.55 | 1.05x |
| nested-64 | 5.13 | 4.97 | 1.03x |
| whitespace | 131.46 | 122.42 | 1.07x |

## coreNormalize

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 1.82 | 2.06 | 0.88x |
| records-128 | 98.57 | 98.58 | 1.00x |
| records-512 | 424.98 | 434.44 | 0.98x |
| numbers-4096 | 197.18 | 198.27 | 0.99x |
| wide-object | 189.92 | 188.66 | 1.01x |
| ascii-string | 242.94 | 240.52 | 1.01x |
| unicode-escapes | 34.91 | 34.06 | 1.02x |
| nested-64 | 3.83 | 3.75 | 1.02x |
| whitespace | 68.19 | 67.68 | 1.01x |

## componentNormalize

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 4.38 | 4.32 | 1.01x |
| records-128 | 106.42 | 106.74 | 1.00x |
| records-512 | 459.83 | 468.29 | 0.98x |
| numbers-4096 | 221.19 | 221.57 | 1.00x |
| wide-object | 218.15 | 215.02 | 1.01x |
| ascii-string | 294.44 | 279.15 | 1.05x |
| unicode-escapes | 42.83 | 41.34 | 1.04x |
| nested-64 | 6.58 | 6.49 | 1.02x |
| whitespace | 128.22 | 118.31 | 1.08x |

## coreInputLength

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 1.04 | 1.21 | 0.86x |
| records-128 | 18.54 | 18.45 | 1.00x |
| records-512 | 74.72 | 75.80 | 0.99x |
| numbers-4096 | 17.95 | 18.09 | 0.99x |
| wide-object | 17.33 | 17.50 | 0.99x |
| ascii-string | 32.38 | 33.01 | 0.98x |
| unicode-escapes | 3.43 | 3.76 | 0.91x |
| nested-64 | 1.05 | 1.06 | 0.99x |
| whitespace | 40.45 | 40.08 | 1.01x |

## componentInputLength

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 3.10 | 3.16 | 0.98x |
| records-128 | 26.21 | 26.27 | 1.00x |
| records-512 | 100.27 | 100.86 | 0.99x |
| numbers-4096 | 44.83 | 41.73 | 1.07x |
| wide-object | 46.82 | 40.06 | 1.17x |
| ascii-string | 83.91 | 72.34 | 1.16x |
| unicode-escapes | 10.48 | 9.43 | 1.11x |
| nested-64 | 3.53 | 3.54 | 1.00x |
| whitespace | 101.49 | 93.06 | 1.09x |
