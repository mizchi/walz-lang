初見の読解試験です。過去の会話や評価を参照しないでください。
読むファイルはこのディレクトリの cheatsheet.md, sample.gwat, contract.md, reader-task.md の4つだけです。リンク先、実装、SPEC、他の出力は読まず、コンパイラも起動しません。
次の設問に日本語で答えて reader.json を作ってください。他のファイルは編集しません。
Q1: main の (result (result i32 i32)) の外側・内側を説明する。
Q2: 外側と内側の result.let の裸の型引数は何を指定するか。それぞれ subject/body/全体の型を挙げる。
Q3: 内側の () はなぜ必要か。$state, $line, $final の有効範囲と型は。
Q4: read-lines、fold の状態、fold 全体、app.process の4つの型は。
Q5: EOF の main の結果と端点処理は。入力の I/O err、app.process の err、在庫不足はそれぞれどうなるか。
Q6: State を move/close しない理由、stream の消費方法、取消しと通常の err の違いを説明する。fold は任意の throw に対する finally か。
Q7: fold の source と initial の評価順と回数は。$final が未使用でも安全か。
Q8: この3資料だけで、台帳参照と配送行数を pair に持ち、成功値を配送行数に変える変更を理解・実装できるか。未定義記号/API推測/実行パス/追加検索の必要性を各々述べる。
JSON形式: {answers:{Q1:string,...,Q8:string}, issues:[{severity:"critical|minor|suggestion",location:string,cause:string,suggestedFix:string}], assumptions:string[], selfAssessment:{G:number,T:number,C:number,D:number}}。
正答してから採点してください。自信がなければ明記し、根拠のない推測で埋めません。構文の煩雑さと資料不足を分けてください。
自己採点は各軸0..4（正答・充足1項目=1点、未回答/部分充足=0点）。G=結果宣言と値型の区別/裸Uの役割/unit束縛/スコープ、T=入力型/状態型/fold結果型/処理関数結果型、C=EOF/I/O伝播/業務エラー区別/GCと所有端点と取消し、D=未定義記号なし/API推測なし/パスとコマンド再現/追加検索不要。この基準自体はラウンド間で変更しない。
