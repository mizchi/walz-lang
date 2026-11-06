import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
import {variants,axes,cases,declarations,rules,render,oracle,baseline} from './cases.mjs';
const hash=text=>createHash('sha256').update(text).digest('hex');
const labelsFor=order=>Object.fromEntries(order.map((variant,i)=>['RSTU'[i],variant]));
const same=(a,b)=>{try{assert.deepEqual(a,b);return true;}catch{return false;}};
export async function prepare(output) {
  await mkdir(output);
  const manifest={schema:'wlt-struct-readability-v1',cases:cases.map(c=>c.id),axes,labels:{readerA:labelsFor(variants),readerB:labelsFor([...variants].reverse())},hashes:{}};
  const put=async(name,text)=>{await writeFile(join(output,name),text,{flag:'wx'});manifest.hashes[name]=hash(text);};
  await mkdir(join(output,'support'));
  for(const name of ['run.mjs','cases.mjs','rubric.md'])await put('support/'+name,await readFile(new URL(name,import.meta.url),'utf8'));
  await put('oracle.json',JSON.stringify(oracle,null,2)+'\n');
  await put('baseline.wlt',baseline());
  const rubric=await readFile(new URL('rubric.md',import.meta.url),'utf8');
  for(const [role,labels] of Object.entries(manifest.labels)) {
    await mkdir(join(output,role));
    const text=`# structの構築を読む\n\nこの資料だけを読む。構文の出自・既存/新規・ユーザーの好みを推測して優劣を付けない。各表記を独立に採点し、最後に用途別の推薦を述べる。全て同じ固定フィールドのGC structを表す。汎用mapやJSON値ではない。全fieldが必要で、重複・余剰・欠落を拒否する。mutableの意味は宣言に従う。暗黙のdefault、spread、keyword値はない。\n\n以下の宣言と束縛の文脈を利用できる。$markは番号をtraceへ記録してその番号を返す。各caseは独立。\n\n\`\`\`wat\n${declarations}\n\`\`\`\n\n${rubric}\n\n`+
      Object.entries(labels).map(([label,variant])=>`## 表記${label}\n\n${rules[variant]}\n\n`+cases.map(c=>`### ${label}.${c.id}\n\n${c.question}\n\n\`\`\`text\n${render(c.expression,variant)}\n\`\`\`\n`).join('\n')).join('\n')+
      `\n## 回答\n\n各表記について6ケースへ回答し、bucketのtotalだけ100へ変えた構築式全体をeditに書く。他の値や型を変えない。各軸の根拠をevidenceへ記す。未記載の規則や迷いはunclearに列挙する。時間や速度を測ったとは主張しない。コンパイラや他の資料は使わない。\n\nJSON形式:\n\`\`\`json\n{"variants":{"R":{"answers":{"user":{"name":"...","age":0,"email":"..."},"bucket":{"count":0,"total":0,"max":0,"totalType":"i32 or i64"},"nested":{"accepted":0,"warnTotal":0,"errorMax":0},"order":{"trace":[],"left":0,"right":0},"cell":{"value":0,"mutable":false},"job":{"index":0,"durationType":"i32 or i64","depsNull":false,"keyFrom":"id or index or duration"}},"edit":"bucketの式全体","scores":{"fields":0,"types":0,"order":0,"layout":0,"changes":0},"evidence":"各軸をcase IDと箇所に対応させた根拠","unclear":[]},"S":{},"T":{},"U":{}},"recommendation":"用途と理由","limitations":["評価の限界"]}\n\`\`\`\nS/T/UもRと同じ完全な形式で書く。説明文の長さでは加点しない。\n`;
    await put(role+'/packet.md',text);
  }
  await writeFile(join(output,'manifest.json'),JSON.stringify(manifest,null,2)+'\n',{flag:'wx'});
  return manifest;
}
export async function verify(directory) {
  const manifest=JSON.parse(await readFile(join(directory,'manifest.json'),'utf8'));
  for(const [name,digest] of Object.entries(manifest.hashes))assert.equal(hash(await readFile(join(directory,name))),digest,'changed input: '+name);
  const truth=JSON.parse(await readFile(join(directory,'oracle.json'),'utf8'));
  const summary={schema:manifest.schema,correct:0,total:0,readers:{},responseHashes:{}};
  for(const [role,labels] of Object.entries(manifest.labels)) {
    const source=await readFile(join(directory,role+'.json'),'utf8'),response=JSON.parse(source);
    summary.responseHashes[role]=hash(source);summary.readers[role]={};
    assert.deepEqual(Object.keys(response.variants).sort(),Object.keys(labels).sort(),'variant labels');
    for(const [label,variant] of Object.entries(labels)) {
      const result=response.variants[label];let correct=0;
      for(const axis of axes)assert.ok(Number.isInteger(result.scores[axis])&&result.scores[axis]>=0&&result.scores[axis]<=4,'invalid score: '+axis);
      assert.ok(typeof result.evidence==='string'&&result.evidence.length>0,'missing evidence');
      assert.ok(Array.isArray(result.unclear),'missing unclear points');
      for(const c of manifest.cases)correct+=Number(same(result.answers[c],truth.answers[c]));
      correct+=Number(typeof result.edit==='string'&&result.edit.replace(/\s+/g,'')===truth.edits[variant].replace(/\s+/g,''));
      summary.total+=manifest.cases.length+1;summary.correct+=correct;
      summary.readers[role][variant]={label,correct,total:manifest.cases.length+1,scores:result.scores,unclear:result.unclear,evidence:result.evidence};
    }
  }
  return summary;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href) {
  const [mode,directory,output]=process.argv.slice(2);assert.ok(directory,'expected run directory');
  if(mode==='prepare'&&!output)await prepare(resolve(directory));
  else if(mode==='verify') {
    const result=await verify(resolve(directory));
    if(output)await writeFile(resolve(output),JSON.stringify(result,null,2)+'\n',{flag:'wx'});
    else console.log(JSON.stringify(result,null,2));
  }else throw new Error('usage: run.mjs prepare NEW_DIRECTORY | verify RUN [NEW_OUTPUT]');
}
