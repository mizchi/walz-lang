# チャネルと構造化並行処理

[English](README.md) | 日本語

`main.walz` と `policy.json` が型付きの境界を宣言し、ホストが `runtime/channels.mjs` の `createChannels()` を提供します。

- `produce`: ホスト側の consumer を明示的に渡す rendezvous stream。
- `pipeline`: ゲストの consumer クロージャが producer と並行して動作し、join の前に writer の close が EOF を送ります。
- `resolved`: 一度だけ確定する Future writer。
- `pending`: 協調的な取消しと解放を検査するため、書込みを待機させます。

`just test-runtime-cli` または `just test-e2e` を実行します。[契約と構文](../../docs/scoped-tasks.md)も参照してください。
