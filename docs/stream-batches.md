# Stream のバッチ転送

```wat
(%match (%stream.read-batch i32 256 (%move $stream))
  (case pair ($next $items)
    (block
      ;; $items は不変 list<i32>。$next は引き続き一つの所有値。
      (%stream.close i32 (%move $next)))))
```

`%stream.read-batch T max stream` は所有する `stream<T>` を消費し、
`tuple<stream<T>, list<T>>` を返す。`max` は i32 で1–65536。結果が空の list なら EOF。
ホストは1要素以上、最大 max 要素までの部分進捗を返せる。指定数がそろうまで待たない。
既存の `%stream.next` は一要素を読む操作のまま。どちらも返された tail を消費または close する。

共有 payload に限定し、要素の順序・GC 値の型・UTF-8 検査を保つ。
Core 単体の ready stream では GC の cons 列からコピーする。
Core＋JSPI と Component では一度の read で Canonical buffer に受け、GC list へ移す。
容量の上限に加えて、Canonical buffer は既存の16 MiB上限以内に制限する。
取消しでは遅延した producer が解放済みの buffer に書かないようにし、reader の終了処理を待つ。

## ホストの protocol

最適化に対応する AsyncIterator は `readMany(max)` を実装し、
`Promise<{items: Array | TypedArray, done: boolean}>` を返せる。
`items` は0–max要素で、空なら done が true でなければならない。
最後の要素と done=true を同時に返すこともできる。
通常の AsyncIterator は next() で一要素ずつ受ける。

`createChannels()` の reader とファイル adapter はこの protocol に対応する。
Component の固定版 jco glue では、tuple/list 自体を payload とする場合も区別できるように
内部の batch marker を使う。生成形式が変われば build を拒否する。

`createChannels().sendMany(writer, values)` は最大65536要素の配列を受け取り、
`Promise<[accepted, status]>` を返す。status は0=全要素を受理、1=終了／取消し。
accepted は buffer または reader へ渡した prefix の要素数で、取消しでも巻き戻さない。
未受理の要素は解放する。空配列は進捗0の成功で、待機 reader を起こさない。
一つの pending read / write、capacity=0 の rendezvous という既存規則を保つ。
WLT import の policy では `(list T) -> (pair i32 i32)` を使い、resource の use と suspends を明示する。

## 検査と計測

`just test-runtime-cli` は ready stream、文字列、unit、両 JS backend の順序・部分受理・
取消し・未解放数0を検査する。511要素を上限13で読む検査では、producer の呼出しは40回。
これはホスト往復数の検査であり、時間の短縮率そのものではない。

[ファイルの例](../examples/wasi-files/README.md) の `sum-batch(size)` は、
byte stream を GC list にまとめて `list.fold` で合計する。
WIT binding を生成した後、`just bench-stream-file /absolute/path/input.bin` で
1 byte と256 bytesを、同じ入力・同じ guest 処理で比較できる。
この計測はファイルの open/close、I/O、ABI 変換、JSPI、GC 処理を含む。

ready-only の順序は既存の `WltDelivery.readMany_preserves_order` と対応させる。
ホストの非同期実装・glue・scheduler の意味保存や liveness の機械証明は含まない。
EOF とファイルの completion future の成功は別であり、読み終わった後も completion を確認する。
