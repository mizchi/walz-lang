# JSON診断と位置指定の型問い合わせ

## check --json

```sh
just check-json examples/result.wlt
cargo run --locked -- check --json path/to/main.wlt
```

stdoutに `wlt-check-v1` のJSONを1つ出力する。通常の `check` と同じ検証を行い、lawがあれば証明も検査する。ホスト関数やゲストプログラムは実行しない。

```json
{"schema":"wlt-check-v1","ok":true,"diagnostics":[]}
```

失敗時は `ok:false` と `diagnostics` の配列を返す。各診断は [既存の診断契約](diagnostics.md) と同じ code/message/primary/related/notes/help を持つ。主位置と関連位置は元ファイルへ対応させる。ソース読み込み・policy読み込み・外部ツールの失敗もJSONにする。

成功は終了コード0、検証・入出力の失敗は1。引数の誤りは2で、使い方をstderrへ出す。`--json` はcheck用のオプションで、従来のテキスト出力は `wlt check FILE` で使う。

## type-at

```sh
just type-at examples/result.wlt 4 8
cargo run --locked -- type-at main.wlt 7 12 --source lib.wlt
```

最初のファイルが検査するコンパイル単位。`--source` を省略するとそのファイルの座標を問い合わせる。指定した場合は、その単位が読み込んだファイルの座標を使う。パスは通常のCLIと同じく現在のディレクトリを基準にする。

行・列は1始まりで、列はUnicode scalarの個数。UTF-8バイト数や表示幅では数えない。読み込み済みソースのスナップショットから位置を引き、生成したモジュールラッパーは問い合わせ対象にしない。
Rust APIで保持したSourceは、元ファイルの変更・削除後も保存済みの内容を問い合わせられる。同梱の `wlt:std` は仮想モジュールであり、ディスク上のstdlibパスを読み込んだファイルとして逆引きするものではない。

成功時は `wlt-type-at-v1` のJSONを返す。

| フィールド | 内容 |
| --- | --- |
| `mode` | `typed` は型付きIR、`wat` は標準WAT/opaque経路 |
| `matches` | その位置に対応する型の候補。見つからなければ空配列 |
| `matches[].function` | ソースに書いた関数・クロージャ名 |
| `specialization` | 汎用関数の具体化名。通常の関数ではnull |
| `ty` | 検査済みの展開後の型 |
| `source_type` | 束縛や戻り値に明示した元の型表記。`$Count`、`$T`、`(%inst ...)`等。注釈がなければnull |
| `location` | 対象式・束縛・戻り値注釈のLabel（span/location/message） |
| `declaration` | ローカル参照・moveに対応する束縛の宣言位置。なければnull |
| `instantiations` | その具体化を要求した位置のLabel配列 |

式、引数・捕捉・let・loop・case等の束縛、関数の戻り値注釈を対象にする。汎用関数の本体を複数の型で具体化していれば候補を分けて返す。型aliasや元の型変数は表示用の情報として保持し、検査の判定には展開後の型を使う。

コメント・空白・対応する式のない位置ではmatchesが空になる。型付きIRがない標準WATでは `mode:wat` と空配列を返し、式ごとの型を推測しない。読み込んでいないファイルや範囲外の座標は `E_SOURCE_POSITION`。

コンパイルエラー時は `ok:false` と `diagnostics` を返し、終了コード1になる。ゼロ座標や引数不足は終了コード2。型問い合わせは既存の型・effect・所有権検査を共有し、`types` と同じくlawの証明は行わない。

Rust APIは `inspect::type_at(&loaded, source_path, line, column, &policy)`。`typing::type_at` は従来どおり展開済み文字列のバイト位置を扱うAPIとして残す。エディタやLSPの通信・キャッシュはこの層に含めない。
