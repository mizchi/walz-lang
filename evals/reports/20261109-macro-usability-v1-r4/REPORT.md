# マクロ実用性評価 v1・式から再開する実装の追試

必須 **20/20** 成功、探索課題の未達 **0件**。前回 [r3](../20261109-macro-usability-v1-r3/REPORT.md) と同じ42入力・同じルーブリックを、新しい処理系で実行した。固定入力のハッシュがすべて同一であることを確認した。

JSON アプリの oracle とポリシー変更、CLI run/test/bench、Core＋JSPI／Component の ABI 解放・取消し、型・effect・所有権の拒否、stdio LSP を含む既存の契約は維持された。

既存の関数に分散した256展開の check 中央値は40.99msから42.68ms。この3回計測では改善を主張しない。同一関数に集中した展開の比較は [v2 r3](../20261109-macro-authoring-v2-r3/REPORT.md) に記録した。

[result.json](result.json)、[assessment.json](assessment.json)、[commands.json](commands.json)、[snapshot](snapshot/)、[work](work/)、[verification.json](verification.json) を保存した。今回は実装が変わるためバイナリハッシュは前回と異なる。既存の保存済みレポート744ファイルと従来の2,830ファイルのハッシュは維持した。

実装者本人による非盲検評価。比較のため入力中の文書も前回と同じ版を使い、現在の仕様書の更新は実行完了後に行った。
