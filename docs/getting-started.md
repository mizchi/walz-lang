# 導入とサンプル

CLI のインストールと `wlt build`・`wlt test`・`wlt bench` の使い方は [CLI ガイド](cli.md) を参照してください。以下はリポジトリ内の開発用タスクとサンプルの説明です。

[mizchi/wlt-lang](https://github.com/mizchi/wlt-lang) は Wasmtime の作業ディレクトリ内の試作から独立した言語リポジトリです。
旧名 gcwat からの変更点と過去の評価記録の扱いは [MIGRATION.md](../MIGRATION.md) にまとめています。

Wasm GC と静的解析を前提に、標準 WAT を基底として型付きマクロを追加する小さな言語の試作です。
拡張の入口には `%` を付け、標準 WAT と区別します。[構文の境界](syntax.md) を参照してください。
基本型と標準関数は `(%use "wlt:core" ...)` から明示的に取り込み、Result や Future は `(%inst $result T E)` / `(%inst $future T)` で参照します。`%let` は複数の束縛をまとめて左から右に初期化できます。[標準モジュールと束縛の例](standard.md) を参照してください。
標準 WAT は書き換えず受け付け、拡張側には不変の `%let`、`%loop` / `%for`、明示 `%capture`、enum / `%match`、Wasm 例外の `%try` / catch を用意します。`if` の結果値は標準 Wasm の意味を使います。
構造体・配列・クロージャ環境は Wasm GC のオブジェクトです。Component 境界の変換用バッファだけを、専用の線形メモリアロケータで管理します。

`$bytes` / `$string` は不変な値で、Component の `list<u8>` / `string` と相互変換できます。
相対パスの `%use` / `%export` で関数・型・tag を選択し、単一 Wasm に静的リンクできます。[モジュールと DCE](static-modules.md) に規約をまとめています。
[文法チートシート](../CHEATSHEET.md) に型・式・所有値・衛生的マクロと実行手順をまとめています。
[初見評価の記録](grammar-eval/REPORT.md) は、在庫台帳の読解と変更を固定ルーブリックで確かめたものです。
[可読性の評価と改善](readability/REPORT.md) では、型の反復・入れ子・配置・変更の追いやすさを別に評価しています。
[evals/](../evals/README.md) に評価を繰り返す課題・基準・スナップショット作成・実行検証をまとめています。`just eval-prepare ID` で新しい評価回を作れます。
型 alias、期待型による型引数の省略、pair の分解 let を使う [行数付き在庫台帳](../examples/apps/stock-ledger/main-counted.wlt) もあります。
[仕様と構文](../SPEC.md#bytes--text-と-abi)、[bytes/text の例](../examples/bytes-text.wlt)、[モジュールの例](../examples/modules.wlt) を参照してください。

```sh
git clone https://github.com/mizchi/wlt-lang.git
cd wlt-lang
just                         # クロージャの例 → 42
just run examples/counter.wlt
just run examples/collections.wlt
just run examples/list.wlt
just run examples/variants.wlt
just run examples/ast-interpreter.wlt
just run examples/bytes-text.wlt
just run examples/modules.wlt
just wat                     # 通常の .wat へ変換
just expand examples/exceptions.wlt  # try/catch などの展開を確認
just build                   # target/program.wasm を生成
just analyze                 # 呼び出しグラフと副作用を JSON で表示
just ir                      # 名前解決・型検査済み IR を JSON で表示
just validate examples/result.wlt  # ソースの静的型検査
just types examples/result.wlt     # 式と束縛の型を JSON で表示
just wit examples/value-types.wlt  # 公開関数の WIT interface を生成
just run-verified              # 所有権と law の証明を通して → 42
just run-verified examples/ready-delivery.wlt # Future/Stream の配送済みデータ → 42
just verify                   # 契約検査のみ。実行しない
just contracts                # 所有権の義務を JSON で表示
just proof                    # 型付き IR から生成した Lean ソース
just proof-check              # 所有権モデルの定理と反例を検証
just test
just test-language            # wlt で書いたテストを実行
just list-tests               # テスト一覧。実行はしない
just wit-check                # async 型の WIT 契約を往復検証
just check
just samples                 # 実行サンプルと期待値を一覧で確認
just probes                  # 未対応機能を再現する検査
```

Rust 1.99 以降と just を使用します。実行と実行テストには Wasm GC 対応の `wasmtime`、WIT のテストには `wasm-tools` が PATH 上に必要です。
契約付き実行・law を含むモジュール・Rust の検証テストには、elan 経由の `lean +v4.34.1` も必要です。
`check`・`wat`・`build`・静的解析には Wasmtime は不要です。law を含まない通常のコンパイルでは Lean も不要です。just を使わず `cargo run --locked -- run examples/closure.wlt` としても実行できます。
生成した Wasm は `wasmtime run -W gc=y,function-references=y,exceptions=y --invoke main target/program.wasm` で実行できます。
ランチャーでは GC・function references・exceptions を明示的に有効にします。

サンプルは [examples/catalog.json](../examples/catalog.json) で管理しています。
30本の実行例に、ソート、行列積、AST 評価、for、例外、値型、所有権、law、Future/Stream、標準 Wasm の数値命令・末尾呼び出しなどを含めています。
追加の31入力、意図した実行時失敗2本、未対応機能3本もテストします。
実装できた範囲と不足機能の考察は [ASSESSMENT.md](../ASSESSMENT.md) を参照してください。
残る機能と実装順は [TODO.md](../TODO.md) で管理しています。項目6.1–6.3と、bytes/text・静的モジュールの初版は実装済みです。

```wat
(module
  (%closure $Adder
    (%capture $base i64)
    (param $x i64)
    (result i64)
    (i64.add (local.get $base) (local.get $x)))

  (func $main (export "main") (result i64)
    (%let $add40 (%closure.new $Adder (i64.const 40))
      (%closure.call $Adder (local.get $add40) (i64.const 2)))))
```

クロージャは宣言ごとの固有型を持ちます。捕捉値を引数で渡して構築し、呼び出し先は型から静的に決まります。
例えば `$Adder` は、捕捉値を保持する GC struct と、環境を第1引数に取る通常の Wasm 関数へ変換されます。
可変状態を捕捉する場合は `counter.wlt` のように、`mut` フィールドを持つオブジェクトへの参照を渡します。

enum は `(type $Result (%variant (ok i64) (error i32)))` と宣言し、`(%variant.new $Result ok (i64.const 42))` で構築します。

```wat
(%match $Result (result i64) (local.get $result)
  (case ok ($value) (local.get $value))
  (case error ($code) (i64.const -1)))
```

payload の型と個数、case の網羅性と重複をコンパイル時に検査します。再帰 enum と、case 内の明示 capture・末尾 `%recur` も使えます。
`examples/ast-interpreter.wlt` はこの形式で実装しています。`examples/result.wlt` は組込みの Result 値型を使います。

```wat
(func $answer (result (%inst $option i64)) (%effect pure)
  (%option.some i64 (i64.const 42)))
```

`(%inst $option T)`・`(%inst $result T E)` は型引数ごとに具体化します。入れ子、unit payload、配列・フィールド・明示 capture に対応します。
これらの格納用 GC オブジェクトの identity はソースで比較できません。payload 自身の GC 参照の性質は保持します。
`(%effect ...)` は reads/writes/allocates/suspends/host と、関数外へ出る throws の上限を検査します。
pure な関数でも値のための物理的な割当てや trap はあり得るため、従来の低水準の解析情報も別に残します。

`(%inst $tuple T U)` も値型として使え、WIT では tuple に対応します。
