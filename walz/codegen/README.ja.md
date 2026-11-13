# Walzのbootstrap codegen

[English](README.md) | 日本語

`compiler.walz` はWalz製parser・[checker](../checker/README.ja.md)・バイナリ生成を
つなぎます。`$compiler.compile(bytes)` は出力bytesまたはソース位置付き診断のResultを
返します。`$compiler.check(bytes)` は出力せずに検査し、関数数を返します。
解析・検査・生成はWalzで実行し、ホストのコンパイラを呼び出しません。

`emit.walz` はcheckerのopaqueな検査済みmoduleだけを受け取ります。
`encoding.walz` は正規形の符号なし32bit・符号付き32/64bitのLEB128を提供します。
`types.walz` はGC再帰グループ・subtype・値型・格納型を出力します。
GC型／関数型・関数・export・codeのsectionに、参照local／結果・型付き制御・
GC命令を出力します。低水準WATの経路ではGC型のindexと明示的な再帰グループの境界を
保ち、型の統合や並べ替えはしません。モジュールfrontendでは、ファイル内indexを
再配置し、明示グループの構成と順序を保持しながら特殊化した型を依存順に並べます。

```sh
just build-selfhost
just selfhost-check examples/selfhost/sum.walz
just selfhost-compile examples/selfhost/sum.walz target/selfhost-sum.wasm
just selfhost-compile examples/selfhost/gc.walz target/selfhost-gc.wasm
just selfhost-compile examples/selfhost/project/main.walz target/selfhost-project.wasm
```

unit/i32/i64の関数、可変local、前方／再帰呼び出し、算術、block/loop、分岐、if、
不変で字句スコープを持つlet、native exportに対応します。GC struct/arrayでは
packed field・再帰グループ・宣言されたsubtype・nullable参照・cast・fill/copyを
扱います。参照の照合は保守的で、再帰型の完全な同値判定ではありません。
未対応の構文は拒否します。
構文木・IR・作業データはWasm GCを使い、出力ABIでは不変のbytesを渡します。

`compile-project`／`check-project`は不変のファイルレコード列を受け取ります。
[frontend](../../docs/selfhost-frontend.md)は静的モジュール・固定構文・GC effectの上限・
最小core・上限付きの単相化・型照会を検査する衛生的な式マクロを扱います。完全な所有権／proof／ABIの
lowering、stage 1/2の再現性検査は今後の課題で、完全なセルフホストではありません。
[契約・対応範囲・上限・検証](../../docs/selfhost.md)を参照してください。
