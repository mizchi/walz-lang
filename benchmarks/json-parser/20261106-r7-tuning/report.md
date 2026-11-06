# JSON parser tuning comparison

v24.21.0 / Apple M5; 9 rotating rounds; medians. Both binaries run in the same process. Input and benchmark-program hashes match. All raw AST hashes and exact normalized outputs match the baseline. Setup and validation are outside timing; automatic GC inside a batch is included. ABI live allocations are zero after each batch.

## residentParse

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 0.37 | 0.25 | 1.49x |
| records-128 | 53.44 | 33.48 | 1.60x |
| records-512 | 220.39 | 139.80 | 1.58x |
| numbers-4096 | 138.20 | 132.17 | 1.05x |
| wide-object | 133.01 | 88.37 | 1.51x |
| ascii-string | 124.14 | 26.83 | 4.63x |
| unicode-escapes | 19.41 | 19.85 | 0.98x |
| nested-64 | 1.26 | 1.03 | 1.21x |
| whitespace | 54.74 | 26.92 | 2.03x |

## coreParseStatus

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 1.38 | 1.27 | 1.08x |
| records-128 | 71.43 | 50.28 | 1.42x |
| records-512 | 287.41 | 203.07 | 1.42x |
| numbers-4096 | 132.89 | 121.92 | 1.09x |
| wide-object | 136.13 | 87.23 | 1.56x |
| ascii-string | 152.79 | 60.36 | 2.53x |
| unicode-escapes | 21.73 | 22.14 | 0.98x |
| nested-64 | 2.22 | 2.15 | 1.03x |
| whitespace | 96.27 | 67.76 | 1.42x |

## componentParseStatus

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 4.19 | 4.03 | 1.04x |
| records-128 | 79.43 | 58.54 | 1.36x |
| records-512 | 314.18 | 227.91 | 1.38x |
| numbers-4096 | 160.54 | 154.52 | 1.04x |
| wide-object | 166.91 | 115.20 | 1.45x |
| ascii-string | 201.49 | 111.87 | 1.80x |
| unicode-escapes | 29.31 | 29.70 | 0.99x |
| nested-64 | 5.04 | 5.02 | 1.00x |
| whitespace | 148.27 | 129.70 | 1.14x |

## coreNormalize

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 2.01 | 1.82 | 1.10x |
| records-128 | 121.76 | 98.41 | 1.24x |
| records-512 | 520.10 | 420.74 | 1.24x |
| numbers-4096 | 210.26 | 197.80 | 1.06x |
| wide-object | 241.06 | 187.69 | 1.28x |
| ascii-string | 346.50 | 236.36 | 1.47x |
| unicode-escapes | 33.32 | 33.69 | 0.99x |
| nested-64 | 3.83 | 3.81 | 1.01x |
| whitespace | 96.14 | 68.19 | 1.41x |

## componentNormalize

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 4.64 | 4.43 | 1.05x |
| records-128 | 129.93 | 108.03 | 1.20x |
| records-512 | 545.44 | 448.25 | 1.22x |
| numbers-4096 | 232.13 | 224.45 | 1.03x |
| wide-object | 272.05 | 216.61 | 1.26x |
| ascii-string | 396.49 | 289.09 | 1.37x |
| unicode-escapes | 41.50 | 41.91 | 0.99x |
| nested-64 | 6.56 | 6.51 | 1.01x |
| whitespace | 152.41 | 130.50 | 1.17x |

## coreInputLength

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 1.10 | 1.25 | 0.88x |
| records-128 | 18.67 | 18.65 | 1.00x |
| records-512 | 74.75 | 74.57 | 1.00x |
| numbers-4096 | 17.96 | 17.96 | 1.00x |
| wide-object | 17.32 | 17.51 | 0.99x |
| ascii-string | 32.42 | 32.49 | 1.00x |
| unicode-escapes | 3.45 | 3.43 | 1.00x |
| nested-64 | 1.03 | 1.04 | 0.99x |
| whitespace | 40.37 | 40.50 | 1.00x |

## componentInputLength

| input | before µs | after µs | speedup |
|---|---:|---:|---:|
| small-object | 3.25 | 3.33 | 0.98x |
| records-128 | 26.67 | 26.29 | 1.01x |
| records-512 | 99.24 | 98.42 | 1.01x |
| numbers-4096 | 42.78 | 45.28 | 0.94x |
| wide-object | 41.98 | 43.30 | 0.97x |
| ascii-string | 74.78 | 84.36 | 0.89x |
| unicode-escapes | 9.85 | 9.94 | 0.99x |
| nested-64 | 3.50 | 3.57 | 0.98x |
| whitespace | 93.55 | 103.55 | 0.90x |
