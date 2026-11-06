# 型検査と静的解析

複数の機能を組み合わせた例として、[依存グラフのワークフロープランナー](../examples/apps/workflow/README.md)があります。
GC グラフ・状態 enum・複数ワーカーへの割当て・循環検出・非同期 I/O を実装し、独立した参照実装と両バックエンドで照合しています。

関数の引数・戻り値などの注釈を境界にして、各式の型、`%let` の束縛型、`case` の payload の型を求めます。
`%let` にも `(%let $answer i64 (i64.const 42) body)` と任意の型注釈を書けます。暗黙の数値型変換は行いません。
型エラーは位置とソース上の型名で表示し、例えば `type mismatch: expected i64, got i32` となります。
引数数、名前候補、型の期待値の根拠、先行する move、effect の原因、opaque の所有 namespace も診断します。
Rust API の `Error::diagnostic()` では分類・主範囲・関連位置・補足・修正方針を取得できます。[診断のデータ契約](diagnostics.md)を参照してください。
CLIの `check --json` と、元ファイルの行・列を指定する `type-at` は [外部ツール向けの検査](tooling.md) を参照してください。

```sh
cargo run --locked -- check examples/ast-interpreter.wlt
cargo run --locked -- types examples/ast-interpreter.wlt
```

型付きコアの `types`・`ir`・`analyze` は名前解決済みの IR を使います。CLI の `check` は標準 WAT も検証します。
`types` は関数の戻り値型、params/captures のローカル ID、全束縛の型と宣言位置、全式の型とソース範囲を返します。
ローカル ID により同名の束縛を区別します。ソース範囲は UTF-8 byte offset の半開区間です。

Rust API では `wlt::check(source)` が型付き IR を返し、`wlt::typing::report(&module)` で型の一覧を取得できます。
`wlt::typing::type_at(&module, offset)` は束縛宣言または最も内側の式の型を返します。
取得した型は `module.type_name(ty)` で `(ref $Expr)` などの表記にできます。
関数シグネチャは明示したままで、省略した result は unit です。全プログラムの型注釈を省略する推論は行いません。
`wlt::validate(source)` は標準 WAT と拡張の両方に対応します。標準 WAT がコアの外にある場合、`types` は検証済み Wasm の関数シグネチャを表示し、式ごとの型は取得できません。
この場合 `Compilation.ir` は None、解析は `opaque=true` でモジュールの全作用を保守的に true とします。

## 静的解析

解析結果は関数とクロージャごとに次の可能性を報告します。

| 項目 | 意味 |
|---|---|
| `reads` | 明示的に可変な GC フィールド・要素を読む |
| `writes` | GC フィールド・要素を書き換える |
| `allocates` | GC オブジェクトを割り当てる |
| `may_trap` | null、境界、整数除算、割当て失敗などで trap する可能性 |
| `may_throw` | Wasm 例外を送出する可能性。関数内で捕捉するものも含む |
| `may_suspend` | 論理的に中断し、他の処理が介在する可能性。未知の外部呼び出しは保守的に true |
| `may_diverge` | ループ・再帰・外部呼び出しにより停止しない可能性 |
| `host` | 外部のホスト関数を呼ぶ |

型付きコアの呼び出し先は静的に決まるので、再帰を含む呼び出しグラフに沿って固定点まで副作用を伝播できます。
不変フィールドの読み取りは `reads` に含めず、nullable な参照なら `may_trap` を付けます。
`%match` は外部 Wasm の不正な enum 表現に備える実行時検査を持つため、`may_trap` を保守的に付けます。
未知の外部関数は全項目を保守的に true とします。固定した event-loop binding は型と作用を検査し、host と必要な suspends を伝播します。ホストへの import を実行する場合は、埋め込み側で実装を与えてください。
CLI の `run` は import のない `main` を実行する最小ランチャーです。

この解析は安全側の近似です。ループや再帰の停止性、添字範囲、割当て失敗が起こらないことは証明しません。
解析情報を使う独自の最適化パスはまだありません。出力後のコード生成・最適化は Wasmtime/Cranelift が担当します。

### 読み書きの対象と、分岐から得られる事実

`just analyze examples/higher-order.wlt` は、従来の副作用フラグに加えて、各関数の memory と facts を出力します。
map では memory.reads が入力の parameter[0]、memory.writes が map 内の allocation を指します。
出力配列の長さが入力と等しいことも保持し、読み取りと書き込みの両方を bounds_proven=true と確認できます。

| 出力 | 内容 |
|---|---|
| memory.reads / writes | 可変な GC フィールド・要素を読む／書く対象。parameter、capture、allocation、unknown とアクセスパス |
| memory.returns | 返す参照・値の内部表現の由来 |
| memory.allocations | 割当ての型と位置、returned/stored/host/thrown の経路で外へ出る可能性 |
| facts | 式ごとの node、UTF-8 のバイト位置、操作、到達可能性、i32 の範囲、nullness、配列長、境界・null 検査の根拠 |

関数間で引数・捕捉・戻り値の別名を代入し、格納した参照、closure や値コンテナ、循環参照、loop/recur と再帰を追います。
引数同士が別物であるとは仮定しません。割当て箇所は呼び出しや反復をまたいで併合し、パスが深さ3を超えた部分は unknown へ広げます。
中断や未知の書き込みの後では、可変フィールドに保持していた参照の情報を広げます。

facts の i32_range は符号なしのビット区間です。折り返しやシフト量のマスクを考慮し、区間で表せない場合は全範囲に戻します。
unsigned の `i < array.len(a)`、non-negative を確認した signed 比較、null 検査、変更されないローカルと配列長の対応を保持します。
ループの持ち回り値は初期値のまま扱わず型の上限へ広げ、枝の合流では両方に共通する条件だけを残します。
`bounds_proven=false` は「未確認」を意味します。各事実はその式の正常な継続についてのもので、割当て失敗など別の trap を除外しません。

これらは解析結果です。既存の pure/effect 上限を緩めたり、検査や割当てを削除したりする経路にはまだ使いません。
非 escape だけを根拠に参照同一性や trap を消すこともできません。i64 の区間や明示的なループ不変条件は今後の範囲です。
Rust API は `regions::analyze(&module)` と `flow::analyze(&module, &function)` です。`analysis::analyze` は両方を含めて報告します。
