# 契約検証の範囲

所有権の正本は [SPEC.md](../SPEC.md#所有値の最小規約) の規約です。実装と契約の変更を別に確認できるよう、型付き IR、抽出する所有契約、Lean の規則を分離します。
参考にしたのは [Bend 2 の law/proof と使用回数の規約](https://github.com/bendlang/bend/blob/3ca3df3209ebb0fac4b80e41dac18b41442e6c58/guide/GUIDE.md) です。BendTT を移植したものではありません。

```sh
just proof-check
just verify examples/verified-laws.wlt
just run-verified examples/ownership.wlt
just run-verified examples/ready-delivery.wlt
just contracts examples/ownership.wlt
just proof examples/verified-laws.wlt
```

Lean は `leanprover/lean4:v4.34.1` に固定します。Rust の verified 経路は、コンパイル時に埋め込んだ Ownership.lean と型付き IR から証明用ソースを生成します。
Lean がない場合も、証明が失敗した場合も、Wasm を出力・実行しません。通常の WAT の経路はこの検証モードとは別に残します。

## 所有権

契約の操作は nop、take、scope、seq、branch、loop、again、stop です。共有値の型は既存の型検査で処理し、契約には所有するローカル ID の利用だけを残します。
宣言的な `Valid` は、利用可能な所有集合、ループへ戻すべき集合、操作、正常終了時に残る集合の関係です。
`check_sound` は、検査が成功した契約がこの関係を満たすことを証明します。`double_take_rejected` は、一度消費した同じスロットを再消費できないことを任意の入力集合について証明します。
loop は再帰的に実行しません。入口の状態と、again の戻す状態の一致を検査します。停止性や、いつか close に到達することは保証しません。

正常系の証人として、同じ所有値をそれぞれの分岐で1回ずつ消費する契約を受理します。破れた証人として、二重消費、片方だけ消費する分岐、外側の所有値を失うループ、消費せず抜けるスコープを拒否します。
Lean ファイル内の example と `tests/verified.rs` が独立した検査器でこれらを確認し、`tests/ownership.rs` は対応する wlt の実コードが拒否されることも検査します。
`sorry` や独自の公理は使いません。Lean の標準論理とカーネルを信頼します。

## アプリケーションの law

law と、その依存関数の実際の型付き IR を BitVec の定義へ変換します。引数を全称量化し、law の i32 の結果が非0である証明を bv_decide で生成・検査します。
有限個の入力を選ぶテストではありません。i32/i64 の全ビットパターンについて、固定幅での折り返しを含む性質を検査します。
`tests/laws.rs` は、加算の実装を減算に変更した場合、また x=65537 だけで壊れる法則でも実行を拒否することを確認します。
符号、拡張、シフト量のマスク、分岐、16進数の折り返しを、検査器と Wasm の実行で確認します。
現在は純粋で非再帰の整数断片だけを扱います。再帰・ループ・除算・GC・所有値・浮動小数点などは未対応として拒否し、仮定で埋めません。

## 配送済み Future/Stream のモデル

Delivery.lean は、ready または closed の Future と、有限列を持つ open または closed の Stream を定義します。
await の後に同じ Future を読めないこと、closed Stream を読めないこと、終端でも open の端点が返ること、next の反復が元の列の take/drop に一致することを証明します。
Stream の EOF は別の completion Future の Result に影響しません。
このモデルも契約付き実行時の Lean ソースへ含めます。読み取りと close の利用回数は既存の所有権の契約で検査します。
`tests/delivery.rs` と `tests/programs/delivery.test.wlt` は、対応する実コードを Wasm GC で実行し、順序・再読・unit・GC 参照・評価回数・終了を照合します。
WIT の future/stream への変換は `tests/wit.rs` で実際に encode/validate します。
モデルは ready-only です。pending、writer、backpressure、外部 I/O、producer task の取消しを証明したものではありません。

## 解析で用いるビット条件

Flow.lean は、signed の添字比較に non-negative の条件を加えると unsigned の境界条件を導けることを、全32ビットの BitVec で検査します。
non-negative を省くと -1 が signed 比較を通る反例と、加算の折り返しも確認します。`just proof-check` で独立して検査します。
`tests/flow.rs` は同じ負の添字で解析が bounds_proven を付けないことと、実際の Wasm が範囲外で trap することを照合します。区間の結果と具体的な折り返し演算も比較します。
この補題は Rust の抽象解釈器の健全性を証明したものではありません。memory/facts は現在診断にだけ使い、生成する Wasm の命令を除去する根拠にはまだしていません。

## 信頼する実装

Tasks.lean は JSPI タスクの要求と終了確認を分ける状態モデルです。実行前の取消し、要求の冪等性、返却後の取消し無効、read 終了前のバッファ解放拒否、資源とホスト解放が残る間の終了確認拒否を証明します。
disposals=1 の状態では終了確認を拒否し、0 にした後で受理する証人を持ちます。対応する Node テストも dispose を barrier で止め、その間は cancel の Promise が確定しないことを検査します。
このモデルは生成する Lean ソースにも含めます。Rust/JS/Wasm との refinement、取消し完了までの時間、外部 producer の停止を証明したものではありません。

証明したのは上の契約モデルと、生成した BitVec 定義についてです。次は証明の対象外です。

- 構文解析、名前解決、Rust の型検査・effect 解析・所有契約の抽出、参照／escape／制御フローの解析。
- 型付き IR と Lean の BitVec 定義の対応、Wasm への lowering の意味保存。
- Lean・Wasm engine、外部資源の操作 protocol、Component ABI adapter とバッファの確保・解放、jco の JSPI glue と局所的な互換性補正。

同じ検査済み IR から証明と実行形式を作り、異常な契約と実コードの両方を回帰テストに残すことで、この境界の食い違いを検出します。
GC の寿命・OOM、実行時 trap、サーバーの停止性、Future/Stream の完了やキャンセルを証明したものではありません。
Component 生成でも同じ所有契約と law を検査しますが、Delivery.lean 自体は ready-only です。外部の pending handle、Canonical ABI の変換、JSPI の中断・再開は Node と Chromium の実行テストで照合する範囲です。

bytes/text の UTF-8 検証・private storage・再帰的な Canonical ABI のコピーと解放、静的 namespace/use の展開、include のファイルローダも TCB です。これらの意味保存は Lean kernel では証明していません。

実際の JS 参照の保持は Lean のカウンタモデルだけでは検査できません。`just memory-check` が所有表の残数、GC 到達性、反復時の保持量とアロケータの再利用を実行検査します。観測と範囲は [MEMORY.md](../MEMORY.md) に記録します。
