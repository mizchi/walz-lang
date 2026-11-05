# WIT からの binding 生成

`wlt bind host.wit policy.json generated` は `bindings.wlt` と `bindings.json` を作る。
WIT world が複数ある場合は、Rust API `wit_bindgen::generate` の world 引数で選ぶ。
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

初版は interface 内の freestanding 関数、s32/s64/f32/f64、string、list<u8>、
option/result、2要素 tuple、future/stream を扱う。空 payload の result/future/stream も扱う。
別名を解決し、未対応の整数型、record、enum、flags、resource、汎用 list、world 直下の関数は
明示的に拒否する。WASI 一式の binding 生成はまだ対応範囲に含めない。

Core＋JSPI は型別の record codec、Component は Canonical ABI の flatten/indirect を使う。
外部 import の instance 型は余分な型 export を要求せず、WIT の引数名を維持する。
Rust の形検査と両 backend の Node 往復検査を `tests/wit_bindings.rs` と
`runtime/wit-bindings.test.mjs` に置く。

取消しは協調的に進む。async の host Promise に `Symbol.asyncDispose`／`Symbol.dispose` があれば取消し時に呼ぶ。
host 操作が完了しない限り、解放と取消しの完了通知も待つ。
