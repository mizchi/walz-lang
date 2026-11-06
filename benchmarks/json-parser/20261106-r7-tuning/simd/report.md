# JSON parser tuning comparison

v24.21.0 / Apple M5; 5 rotating rounds; medians. Both binaries run in the same process. Input and benchmark-program hashes match. All raw AST hashes and exact normalized outputs match the baseline. Setup and validation are outside timing; automatic GC inside a batch is included. ABI live allocations are zero after each batch.

## residentParse

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 0.25 | 0.30 | 0.82x |
| records-128 | 37.67 | 43.95 | 0.86x |
| records-512 | 139.58 | 166.71 | 0.84x |
| numbers-4096 | 132.79 | 135.58 | 0.98x |
| wide-object | 86.06 | 114.33 | 0.75x |
| ascii-string | 30.26 | 24.12 | 1.25x |
| unicode-escapes | 19.20 | 23.62 | 0.81x |
| nested-64 | 1.02 | 1.03 | 0.99x |
| whitespace | 26.68 | 26.72 | 1.00x |

## coreParseStatus

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 1.29 | 1.31 | 0.98x |
| records-128 | 52.22 | 58.15 | 0.90x |
| records-512 | 202.10 | 224.75 | 0.90x |
| numbers-4096 | 122.41 | 120.16 | 1.02x |
| wide-object | 87.11 | 99.91 | 0.87x |
| ascii-string | 62.80 | 57.07 | 1.10x |
| unicode-escapes | 21.59 | 25.53 | 0.85x |
| nested-64 | 2.15 | 2.14 | 1.00x |
| whitespace | 68.03 | 67.35 | 1.01x |

## componentParseStatus

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 3.96 | 3.99 | 0.99x |
| records-128 | 62.08 | 67.34 | 0.92x |
| records-512 | 225.21 | 247.63 | 0.91x |
| numbers-4096 | 148.40 | 151.56 | 0.98x |
| wide-object | 112.88 | 131.25 | 0.86x |
| ascii-string | 114.48 | 102.81 | 1.11x |
| unicode-escapes | 28.51 | 33.20 | 0.86x |
| nested-64 | 4.88 | 4.89 | 1.00x |
| whitespace | 124.70 | 132.65 | 0.94x |

## coreNormalize

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 1.85 | 1.85 | 1.00x |
| records-128 | 105.55 | 111.76 | 0.94x |
| records-512 | 421.21 | 444.40 | 0.95x |
| numbers-4096 | 197.90 | 196.70 | 1.01x |
| wide-object | 186.65 | 198.43 | 0.94x |
| ascii-string | 238.54 | 234.24 | 1.02x |
| unicode-escapes | 33.20 | 37.57 | 0.88x |
| nested-64 | 3.70 | 3.65 | 1.01x |
| whitespace | 68.02 | 68.31 | 1.00x |

## componentNormalize

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 4.25 | 4.41 | 0.97x |
| records-128 | 113.08 | 119.43 | 0.95x |
| records-512 | 443.96 | 470.88 | 0.94x |
| numbers-4096 | 217.99 | 219.80 | 0.99x |
| wide-object | 212.90 | 228.87 | 0.93x |
| ascii-string | 300.86 | 282.97 | 1.06x |
| unicode-escapes | 40.28 | 44.49 | 0.91x |
| nested-64 | 6.31 | 6.31 | 1.00x |
| whitespace | 127.44 | 133.86 | 0.95x |

## coreInputLength

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 1.05 | 1.05 | 1.00x |
| records-128 | 19.83 | 19.36 | 1.02x |
| records-512 | 74.24 | 74.63 | 0.99x |
| numbers-4096 | 17.90 | 17.85 | 1.00x |
| wide-object | 17.32 | 17.38 | 1.00x |
| ascii-string | 32.01 | 32.20 | 0.99x |
| unicode-escapes | 3.37 | 3.71 | 0.91x |
| nested-64 | 1.02 | 1.00 | 1.01x |
| whitespace | 40.04 | 39.94 | 1.00x |

## componentInputLength

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 3.22 | 3.17 | 1.02x |
| records-128 | 27.78 | 27.99 | 0.99x |
| records-512 | 97.03 | 97.63 | 0.99x |
| numbers-4096 | 41.43 | 46.78 | 0.89x |
| wide-object | 39.40 | 45.09 | 0.87x |
| ascii-string | 78.17 | 85.54 | 0.91x |
| unicode-escapes | 9.95 | 10.42 | 0.95x |
| nested-64 | 3.45 | 3.45 | 1.00x |
| whitespace | 96.77 | 105.53 | 0.92x |
