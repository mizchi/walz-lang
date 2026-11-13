# Walzのbootstrap checker

[English](README.md) | 日本語

Walzで書いたcheckerで、ホストのコンパイラをimportせずに動作します。
対象はbootstrapコンパイラのunit/i32/i64とGCの範囲で、Wasm全体やWalz全体ではありません。

- `ir.walz` は未検証で不変なGC型・フィールド・命令・関数・exportのレコードを定義します。
- `operators.walz` は対応するopcode・型・引数数・演算子の別名を定義します。
- `gc-operators.walz` はGC命令の表、`syntax.walz` は共通の構文アクセスを提供します。
- `type-env.walz` はソースの型・フィールド・再帰グループを解決します。
- `gc-types.walz` は未検証のGC型・フィールドの変性・参照のsubtypeを検査します。
- `checker.walz` は名前解決・型推論・構文木からの変換を行います。
- `stack.walz` は独立してoperand/control stack、シグネチャ、index、即値、
  export名、資源上限を検査します。
- `gc-effects.walz` は可変GC操作のeffectを和集合し、直接呼出しを固定点まで伝播して、
  pure／reads／writes／allocatesの上限を検査します。

`$checker.check(syntax)`、`$checker.validate(functions, exports)`、
`$checker.validate-gc(types, functions, exports)` は
`$result<(ref $module-ir), (ref $diagnostic)>` を返します。検査成功時だけ、
codegenが受け付けるopaqueなmoduleを作ります。公開された未検証レコードや
検査済みmoduleの読み取り用getterから、検査結果を書き換えることはできません。

可変local、ラベル付きblock/loop、分岐、return、if、select、単一束縛の不変letに
対応します。到達不能コードもWasmのstack polymorphismに従って検査します。
GC struct/arrayはpacked field、再帰グループ、宣言されたsubtype、nullable参照、
cast、array fill/copyに対応します。不変値の構築・読取りはpureです。
non-null letはnullableな実localと、検査された字句スコープの初期化を使います。

参照の照合は宣言indexと親の関係を使います。再帰型の完全な同値判定は未実装なので、
有効なWasmを拒否する場合もあります。浮動小数点・SIMD・import・関数／外部参照・
完全なeffect/所有権/proof検査は未対応です。
別の[frontend](../../docs/selfhost-frontend.md)が静的モジュール・固定構文・最小core・
上限付きの型／関数ジェネリクスと、独立して型照会を検査する衛生的なマクロを展開し、この独立したvalidatorへ渡します。
明示localはdefaultableな
型に限定し、non-null localは初期化解析を追加するまで拒否します。

```sh
just build-selfhost
just selfhost-check examples/selfhost/sum.walz
just selfhost-check examples/selfhost/gc.walz
just selfhost-check examples/selfhost/project/main.walz
just test-selfhost
```

[APIの契約・構文・診断・差分検証](../../docs/selfhost.md)を参照してください。
