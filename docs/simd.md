# v128 と SIMD

`v128` は Wasm の128ビット値型をそのまま使うプリミティブ。import は不要です。`i32x4` などは別の型ではなく、同じビット列を各命令がどう解釈するかを指定します。WLT 専用の SIMD 構文は追加せず、標準 WAT の命令名を使います。

```wat
(module
  (func $main (export "main") (result i32) (%effect pure)
    (%let $v v128 (v128.const i32x4 40 0 0 0)
      (i32x4.extract_lane 0
        (i32x4.add (local.get $v) (i32x4.splat (i32.const 2)))))))
```

`just run examples/simd.wlt` は42を返します。型付き IR で検査し、対応する Wasm SIMD 命令へ直接出力します。SIMD 演算のためのランタイムやヒープ割当ては追加しません。

## 命令と型

| 操作 | 例・型の規約 |
| --- | --- |
| 定数 | `v128.const i8x16 …` / `i16x8 …` / `i32x4 …` / `i64x2 …` / `f32x4 …` / `f64x2 …`。形に応じて16/8/4/2/4/2個のlane値 |
| 複製 | `(i32x4.splat i32-expr)` → `v128`。`i8x16` / `i16x8` も入力は `i32` |
| 取り出し | `(i32x4.extract_lane 0 vector)` → `i32`。`i8x16` / `i16x8` は `_s` / `_u` を指定 |
| 置換 | `(i64x2.replace_lane 1 vector i64-expr)` → `v128` |
| 算術・比較・変換 | `i32x4.add`、`f32x4.mul`、`i16x8.eq`、`i32x4.trunc_sat_f32x4_s` など。形の変換後も型は `v128` |
| ビット操作 | `v128.and` / `or` / `xor` / `not`、3引数の `v128.bitselect` |
| シフト | `(i64x2.shl vector i32-expr)`。シフト量は各形とも `i32` |
| 集約 | `v128.any_true`、整数laneの `all_true` / `bitmask` → `i32` |
| 並べ替え | `i8x16.shuffle` は0〜31の固定添字16個と `v128` 2値。`i8x16.swizzle` は動的な添字ベクトル |

lane番号と shuffle の添字は、式ではなく命令の即値です。範囲、定数の形・lane数、引数の数と型を検査します。対応命令は固定した `wasmparser` の通常 SIMD 命令一覧から取り、署名と即値を Wasm validator と照合します。

整数演算の折り返し、飽和演算、比較結果のビットマスク、浮動小数点や NaN の扱いは、[Wasm のベクトル命令](https://webassembly.github.io/spec/core/valid/instructions.html#vector-instructions)に従います。WLT 独自の演算規則は加えません。

## GC・ジェネリクス・解析

`v128` は共有可能な値で、`shared` / `nonunit` 制約を満たします。関数の引数・戻り値、`%let`、GC の struct/array フィールド、明示したクロージャ捕捉、`list` / `option` などの型引数に使えます。ベクトル自体に参照 identity や所有権の move はありません。可変フィールドへの読み書きやコンテナの割当ては、既存の effect 規則に従います。

対応する非メモリ SIMD 命令自体は pure で trap しません。オペランドは左から右に一度ずつ評価し、呼び出し・読み書き・例外・中断などの作用をそのまま引き継ぎます。`%effect pure` でオペランドの作用を隠すことはできません。

型・effect・所有権・アクセス先の解析を維持しますが、ベクトルの値域推論や SIMD の代数的な簡約は追加していません。現在の Lean の `%law` モデルも SIMD を扱わず、対象外の式として拒否します。

## ABI と対応範囲

WIT / Component の値型と [WebAssembly JavaScript API](https://webassembly.github.io/spec/js-api/index.html)には、`v128` を直接渡す値表現がありません。WIT 生成、Component、Core＋JSPI の import/export 境界では、直接の `v128` と `list<v128>` / `future<v128>` などの入れ子を拒否します。スカラー、スカラーのlist、bytesに明示的に変換するラッパーを使います。

Wasm 内部の関数・GC格納・Future/Streamのpayloadには `v128` を使えます。内部で作った値を中断の前後で保持し、スカラーへ取り出して返すこともできます。Core Wasm 単体の生成では、Wasmの `v128` 署名を出力できます。

型付きコアの対象は通常 SIMD の**非メモリ命令**です。線形メモリの `v128.load` / `store` / load/store lane と Relaxed SIMD は、従来の標準 WAT 経路で扱います。その経路の解析は `opaque` となり、型付きコアの effect・所有権契約を保証しません。対応外の命令を混ぜて契約検査を迂回することはできません。

実例は [最小例](../examples/simd.wlt)、[ABI・非同期の例](../tests/component-fixtures/simd.wlt)。検査は `just test-simd`、ブラウザは `pnpm exec playwright test runtime/e2e/simd.spec.mjs` で再実行できます。

バルク演算の実装とJS・jsimdとの比較は [SIMDカーネルと測定](simd-kernels.md) を参照してください。
