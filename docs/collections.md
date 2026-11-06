# 辞書とlist builder

`wlt:std` の通常のWLTライブラリとして提供する。型引数は共有可能・非unitに制限し、具体化後の型・effect・所有権・opaque検査を行う。GCの寿命管理を使い、ホストの管理表や新しい言語構文は追加しない。

## textキーのdict

```text
(%import "wlt:std"
  (type $dict.Dict)
  (func $dict.new) (func $dict.len) (func $dict.get)
  (func $dict.set) (func $dict.remove) (func $dict.clear) (func $dict.entries))

(ref (%inst $dict.Dict T))
```

| 操作 | 結果・動作 | effect上限 |
| --- | --- | --- |
| `new<T>()` | 空の辞書参照 | allocates |
| `len<T>(dict)` | 異なるキー数のi32 | reads |
| `get<T>(dict,key)` | 存在すればsome(T)、なければnone | reads |
| `set<T>(dict,key,value)` | 更新前の値のOption。新しいキーならnone | reads/writes/allocates |
| `remove<T>(dict,key)` | 削除した値のOption。なければnone | reads/writes |
| `clear<T>(dict)` | 全要素を除去。バケット容量は保持 | reads/writes |
| `entries<T>(dict)` | 現在の全キー・値を `list<pair<text,T>>` のスナップショットで返す | reads/writes/allocates |

キーは不変textのUTF-8バイト列として比較する。空文字・NUL・Unicodeを扱い、正規化はしない。値がOptionの場合も「キーなし」と「noneを格納したキー」を区別する。

内部は16バケットから始まるFNV-1aの連鎖ハッシュ表。負荷が1に達した後の新規挿入で容量を倍にする。既存キーの更新ではキー数を増やさない。衝突時は完全なキー比較を行う。entriesの列挙順は規定しない。
entriesの完成後に辞書を更新・削除・clearしても、返したlistのキー・値は変わらない。値が可変GC参照なら参照先の同一性を保つ。構築にはlist builderを使い、内部の書き込みもeffect上限に含める。

探索はキーのバイト長と同じバケット内の要素数に依存し、最悪時は線形。拡張は全要素の再ハッシュ、clearはバケット容量、entriesは容量と要素数に比例する。容量上限や割当て失敗はWasmのtrap。辞書は共有できる可変GCオブジェクトなので、別名からも更新を観測する。

## 不変listを作るbuilder

```text
(%import "wlt:std"
  (type $list-builder.Builder)
  (func $list-builder.new) (func $list-builder.push) (func $list-builder.extend)
  (func $list-builder.len) (func $list-builder.finish) (func $list-builder.clear))

(ref (%inst $list-builder.Builder T))
```

`new<T>()` は空のbuilder、`push<T>(b,value)` は末尾追加、`extend<T>(b,list)` は先頭から順に追加する。`len<T>(b)` は追加済みの要素数。`finish<T>(b)` は追加順の不変listを返して空へ戻し、`clear<T>(b)` は要素を捨てて空へ戻す。

newはallocates、lenはreads、clearはwrites、push/extend/finishはreads/writes/allocatesを上限にする。非公開のGCノードを使いpushは定数時間、extendとfinishは要素数に比例する。clearはノードへの参照を外すだけで、GCが到達不能になったノードを回収する。

完成したlistはbuilderの再利用・clearから独立する。要素に可変GC参照を入れた場合はその参照の同一性を保ち、参照先を深くコピーしない。Future/Stream/own/borrowを含む所有・借用要素と、直接のunit要素は拒否する。

完全なシグネチャは [生成API一覧](stdlib-api.md) を参照。実行例は [collections.wlt](../tests/component-fixtures/collections.wlt)。`just test-collections` はWasmtimeと両JSバックエンドで、独立したMapとの照合、Unicode、衝突・拡張・削除、スナップショット、ABI live=0を検査する。
