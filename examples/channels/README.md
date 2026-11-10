# Channels and structured concurrency

English | [日本語](README.ja.md)

`main.walz` and `policy.json` declare the typed boundary; the host supplies `runtime/channels.mjs`'s `createChannels()`.

- `produce`: a rendezvous stream with an explicitly supplied host consumer.
- `pipeline`: a guest consumer closure runs concurrently with its producer; writer close sends EOF before join.
- `resolved`: a one-shot Future writer.
- `pending`: a blocked write used to verify cooperative cancellation and cleanup.

Run `just test-runtime-cli` or `just test-e2e`. See [contracts and syntax](../../docs/scoped-tasks.md).
