初見のコード変更試験です。過去の会話や評価を参照しないでください。
読む資料はこのディレクトリの cheatsheet.md, sample.gwat, contract.md, writer-task.md の4つだけです。実装、SPEC、リンク先、他の評価は読まないでください。CLI check/types/fmt の診断は利用できます。
sample.gwat をもとに writer/candidate.gwat を作る。成功時の値を、read-lines が配送した行数に変更してください。空行と # 行も数え、EOF は数えません。JSON 出力、業務エラー後の継続、I/O err、取消しの動作を維持します。
制約: stream.fold-result と result.let を使う。fold の状態 S は (pair (ref $stock.State) i32) とし、参照とカウンタを状態として受け渡す。stock.new(4096) を開始時に一度だけ呼び、全行で再利用する。可変カウンタや補助関数で pair を回避しない。program.gwat の include はそのまま。言語実装や既存サンプルは変更しない。
書込先は writer/candidate.gwat と writer/report.json のみ。writer/program.gwat は親が用意した依存橋渡しで、変更・閲覧しないでください。
cwd: /Users/mz/ghq/github.com/bytecodealliance/wasmtime/examples/gcwat
check: cargo run --locked --quiet -- check /Users/mz/ghq/github.com/bytecodealliance/wasmtime/examples/gcwat/target/grammar-eval/round-4/writer/candidate.gwat
目安: ツール呼出し12回以内、コンパイル修正2回以内。足りなければ未完でも報告してください。独自に依存実装を探索しません。
report.json: {completed:boolean,checks:[{command:string,exitCode:number,diagnostic:string}],repairs:string[],ambiguities:[{severity:string,location:string,problem:string,suggestedFix:string}],assumptions:string[],extraDocumentsRead:string[]}。
模範解答を推測して採点せず、実際に書いたコードと診断に基づいて報告してください。
