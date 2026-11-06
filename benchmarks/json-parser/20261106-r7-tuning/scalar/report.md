# JSON parser tuning comparison

v24.21.0 / Apple M5; 5 rotating rounds; medians. Both binaries run in the same process. Input and benchmark-program hashes match. All raw AST hashes and exact normalized outputs match the baseline. Setup and validation are outside timing; automatic GC inside a batch is included. ABI live allocations are zero after each batch.

## residentParse

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 0.37 | 0.25 | 1.49x |
| records-128 | 51.47 | 33.60 | 1.53x |
| records-512 | 216.85 | 140.99 | 1.54x |
| numbers-4096 | 135.48 | 133.28 | 1.02x |
| wide-object | 131.71 | 87.89 | 1.50x |
| ascii-string | 122.14 | 30.79 | 3.97x |
| unicode-escapes | 19.24 | 19.24 | 1.00x |
| nested-64 | 1.24 | 1.03 | 1.21x |
| whitespace | 53.36 | 26.77 | 1.99x |

## coreParseStatus

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 1.33 | 1.28 | 1.04x |
| records-128 | 70.59 | 50.08 | 1.41x |
| records-512 | 284.14 | 202.38 | 1.40x |
| numbers-4096 | 129.66 | 123.03 | 1.05x |
| wide-object | 136.89 | 87.83 | 1.56x |
| ascii-string | 156.14 | 64.69 | 2.41x |
| unicode-escapes | 21.75 | 21.70 | 1.00x |
| nested-64 | 2.23 | 2.15 | 1.04x |
| whitespace | 94.43 | 67.45 | 1.40x |

## componentParseStatus

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 4.28 | 3.88 | 1.10x |
| records-128 | 78.70 | 58.00 | 1.36x |
| records-512 | 307.98 | 226.64 | 1.36x |
| numbers-4096 | 153.78 | 151.00 | 1.02x |
| wide-object | 164.12 | 113.07 | 1.45x |
| ascii-string | 205.58 | 110.84 | 1.85x |
| unicode-escapes | 28.94 | 28.14 | 1.03x |
| nested-64 | 5.03 | 4.91 | 1.03x |
| whitespace | 151.67 | 119.00 | 1.27x |

## coreNormalize

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 2.03 | 1.80 | 1.13x |
| records-128 | 119.99 | 99.39 | 1.21x |
| records-512 | 515.16 | 424.26 | 1.21x |
| numbers-4096 | 204.12 | 194.56 | 1.05x |
| wide-object | 240.19 | 186.58 | 1.29x |
| ascii-string | 346.03 | 241.37 | 1.43x |
| unicode-escapes | 33.29 | 33.37 | 1.00x |
| nested-64 | 3.76 | 3.66 | 1.03x |
| whitespace | 92.36 | 67.66 | 1.37x |

## componentNormalize

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 4.68 | 4.32 | 1.08x |
| records-128 | 128.43 | 106.49 | 1.21x |
| records-512 | 542.51 | 445.02 | 1.22x |
| numbers-4096 | 221.78 | 218.58 | 1.01x |
| wide-object | 266.87 | 209.32 | 1.27x |
| ascii-string | 395.13 | 285.09 | 1.39x |
| unicode-escapes | 40.26 | 40.32 | 1.00x |
| nested-64 | 6.43 | 6.42 | 1.00x |
| whitespace | 148.23 | 116.02 | 1.28x |

## coreInputLength

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 1.10 | 1.21 | 0.91x |
| records-128 | 18.62 | 18.68 | 1.00x |
| records-512 | 74.68 | 73.94 | 1.01x |
| numbers-4096 | 17.80 | 17.91 | 0.99x |
| wide-object | 17.40 | 17.54 | 0.99x |
| ascii-string | 32.10 | 32.83 | 0.98x |
| unicode-escapes | 3.33 | 3.40 | 0.98x |
| nested-64 | 1.02 | 1.03 | 0.99x |
| whitespace | 40.22 | 40.34 | 1.00x |

## componentInputLength

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 3.41 | 3.34 | 1.02x |
| records-128 | 26.22 | 26.28 | 1.00x |
| records-512 | 96.73 | 98.98 | 0.98x |
| numbers-4096 | 43.19 | 44.17 | 0.98x |
| wide-object | 40.78 | 40.42 | 1.01x |
| ascii-string | 80.26 | 76.42 | 1.05x |
| unicode-escapes | 9.96 | 9.40 | 1.06x |
| nested-64 | 3.48 | 3.47 | 1.00x |
| whitespace | 93.44 | 94.05 | 0.99x |
