# WIT からの binding 生成

`wlt bind host.wit policy.json generated [--world name]` は `bindings.wlt`、`bindings.json`、共有型の `types.wlt` と実装用の `exports.wlt` を作る。
WIT world が複数ある場合は `--world`、Rust API では `wit_bindgen::generate` の world 引数で選ぶ。
生成したモジュールから必要な関数だけを named import し、コンパイル時に
`--bindings generated/bindings.json` を指定する。JS の build API では第3引数に
`{bindings: "generated/bindings.json"}` を渡す。

policy の例:

```json
{"imports":[{"module":"example:host/service@1.0.0","name":"echo","suspends":false,"capability":"echo"}]}
```

型は WIT、権限と suspension は policy が指定する。async の有無が WIT と食い違う場合、
policy に重複・不足・余分な項目がある場合、生成を拒否する。固定の組込み契約は上書きできない。
生成ソースを取り込むだけでは信頼しない。ホスト側も `imports` に関数を明示的に供給する。

interface 内の freestanding 関数、s32/s64/f32/f64、u8/u16/u32/u64、string、list<u8>、
option/result、1要素以上の tuple、共有要素の list<T>、future/stream を扱う。空 payload の result/future/stream も扱う。
record、enum、payload 付き variant、bool/char/s8/s16、1–32個の flags も扱い、別名を解決する。import された resource の own/borrow、constructor/method にも対応する。resource は native WIT Component 境界で扱う。所有要素の list と world 直下の import 関数は明示的に拒否する。再帰型、可変・空 record も境界に出せない。

生成した `%abi.type` が、GC struct / enum と nominal unsigned / flags の明示的な値 ABI を指定する。通常の GC ref を暗黙にコピーする機能ではない。unsigned の source storage は i32/i64 のビット列で、`%newtype.wrap` / unwrap が必要。Core＋JSPI の codec は JS からの u8/u16/u32/u64 の範囲と enum/flags の値を検査する。Component の JS 数値変換は固定版 jco の Canonical ABI に従う。

`exports.wlt` は署名と同じ共有型を使う実装の雛形。`unreachable` を実装に置き換える。interface 関数は `interface-function` に平坦化する。対応する world は `wlt build main.wlt --target component --wit api.wit --world guest` で、元の interface 名・型 export・引数名を持つ Component にできる。実装の署名と nominal 型の対応を検査し、private delivery import を内部化する。native world adapter は共有 payload の async endpoint と、import した resource の own/borrow を Canonical ABI に接続する。型付き binding を使用し、旧来の数値 resource factory は拒否する。詳細と制限は [native worlds](native-worlds.md) を参照。WASI 一式の resource world binding は後続とし、ファイルは [版を固定した値 adapter](wasi-files.md) で接続する。

Core＋JSPI は型別の record codec、Component は Canonical ABI の flatten/indirect を使う。
外部 import の instance 型は余分な型 export を要求せず、WIT の引数名を維持する。
Rust の形検査と両 backend の Node 往復検査を `tests/wit_bindings.rs` と
`runtime/wit-bindings.test.mjs` に置く。

WLT SDK を使う両 JS backend では、取消しは協調的に進む。async の host Promise に `Symbol.asyncDispose`／`Symbol.dispose` があれば取消し時に呼ぶ。
host 操作が完了しない限り、解放と取消しの完了通知も待つ。

追加の ABI 指定は `(%abi.type $Bool (bool))`、`(char)`、`(s8)`、`(s16)`、不変 struct の `(tuple)`、各 case が最大1 payload の `(variant "empty" "value")`。char は Unicode scalar value、bool は0/1、小さい整数は範囲を検査する。空 tuple と再帰型は固定版 Component validator に合わせて拒否する。

## ABI 型の実装上限

型グラフのレイアウト・展開数・境界の型表現は検査時にメモ化する。同じ子型を繰り返す DAG を再帰的な木として再検査しない。1 型につき inline サイズ 16 MiB、flatten 1024 スロット、展開した型表現 64 KiB、深さ 64 未満、検査全体の型表現 8 MiB を上限とし、超過は `E_ABI_LIMIT`。サイズ計算は checked arithmetic を使う。adapter の関数引数全体にも 1024 スロット・16 MiB の上限を適用する。この引数制限は通常の Core WAT 関数には適用しない。通常の非公開 GC 型を値 ABI に暗黙変換しない。大きいデータは inline record の重複ではなく `list` / `bytes` / `stream` で渡す。
