# JSON parser tuning comparison

v24.21.0 / Apple M5; 5 rotating rounds; medians. Both binaries run in the same process. Input and benchmark-program hashes match. All raw AST hashes and exact normalized outputs match the baseline. Setup and validation are outside timing; automatic GC inside a batch is included. ABI live allocations are zero after each batch.

## residentParse

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 0.24 | 0.29 | 0.83x |
| records-128 | 33.72 | 40.21 | 0.84x |
| records-512 | 140.90 | 166.25 | 0.85x |
| numbers-4096 | 135.17 | 111.90 | 1.21x |
| wide-object | 86.97 | 77.47 | 1.12x |
| ascii-string | 30.29 | 29.60 | 1.02x |
| unicode-escapes | 19.62 | 17.98 | 1.09x |
| nested-64 | 1.02 | 1.03 | 0.99x |
| whitespace | 26.69 | 27.44 | 0.97x |

## coreParseStatus

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 1.25 | 1.26 | 0.99x |
| records-128 | 50.34 | 56.86 | 0.89x |
| records-512 | 203.22 | 229.76 | 0.88x |
| numbers-4096 | 127.72 | 113.61 | 1.12x |
| wide-object | 87.25 | 83.23 | 1.05x |
| ascii-string | 62.47 | 62.56 | 1.00x |
| unicode-escapes | 21.61 | 21.87 | 0.99x |
| nested-64 | 2.17 | 2.16 | 1.00x |
| whitespace | 70.44 | 68.26 | 1.03x |

## componentParseStatus

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 3.88 | 3.99 | 0.97x |
| records-128 | 58.19 | 65.09 | 0.89x |
| records-512 | 229.03 | 252.80 | 0.91x |
| numbers-4096 | 151.20 | 144.49 | 1.05x |
| wide-object | 114.69 | 104.27 | 1.10x |
| ascii-string | 106.24 | 104.44 | 1.02x |
| unicode-escapes | 28.59 | 28.69 | 1.00x |
| nested-64 | 5.04 | 4.94 | 1.02x |
| whitespace | 125.71 | 120.27 | 1.05x |

## coreNormalize

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 1.80 | 1.89 | 0.95x |
| records-128 | 98.15 | 106.97 | 0.92x |
| records-512 | 427.07 | 460.12 | 0.93x |
| numbers-4096 | 201.12 | 196.55 | 1.02x |
| wide-object | 187.37 | 175.02 | 1.07x |
| ascii-string | 238.02 | 238.18 | 1.00x |
| unicode-escapes | 36.14 | 33.22 | 1.09x |
| nested-64 | 3.84 | 3.65 | 1.05x |
| whitespace | 68.01 | 68.33 | 1.00x |

## componentNormalize

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 4.26 | 4.59 | 0.93x |
| records-128 | 106.30 | 114.64 | 0.93x |
| records-512 | 451.97 | 498.33 | 0.91x |
| numbers-4096 | 220.00 | 224.43 | 0.98x |
| wide-object | 216.01 | 197.17 | 1.10x |
| ascii-string | 287.55 | 284.82 | 1.01x |
| unicode-escapes | 40.29 | 43.20 | 0.93x |
| nested-64 | 6.61 | 6.40 | 1.03x |
| whitespace | 140.11 | 114.40 | 1.22x |

## coreInputLength

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 1.06 | 1.07 | 1.00x |
| records-128 | 18.62 | 18.50 | 1.01x |
| records-512 | 74.59 | 74.37 | 1.00x |
| numbers-4096 | 18.37 | 18.65 | 0.98x |
| wide-object | 17.34 | 17.31 | 1.00x |
| ascii-string | 32.54 | 32.32 | 1.01x |
| unicode-escapes | 3.46 | 3.78 | 0.91x |
| nested-64 | 1.03 | 1.01 | 1.02x |
| whitespace | 40.03 | 40.04 | 1.00x |

## componentInputLength

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 3.12 | 3.31 | 0.94x |
| records-128 | 25.87 | 26.00 | 0.99x |
| records-512 | 96.96 | 97.80 | 0.99x |
| numbers-4096 | 42.50 | 45.99 | 0.92x |
| wide-object | 44.03 | 40.48 | 1.09x |
| ascii-string | 75.95 | 73.80 | 1.03x |
| unicode-escapes | 9.69 | 9.81 | 0.99x |
| nested-64 | 3.46 | 3.48 | 0.99x |
| whitespace | 98.23 | 89.56 | 1.10x |
