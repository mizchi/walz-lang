import {parse} from 'acorn';

const marker='walz-native-host-results-v1';
function children(node) {
  return Object.values(node).flatMap(v=>Array.isArray(v)?v.filter(n=>n&&typeof n.type==='string'):v&&typeof v.type==='string'?[v]:[]);
}
function nodes(root) {
  const out=[],pending=[root];
  while(pending.length){const n=pending.pop();out.push(n);pending.push(...children(n));}
  return out;
}
// jco 0.23.0 serializes a host async result before its hostProvided branch,
// then AsyncSubtask lowers it again into the caller's result pointer. Retire
// only the first, unused copy. Preserve jco's guarded async realloc scheduling;
// moving the completion branch before that scheduling is not equivalent.
export function nativeHostResults(source) {
  if(!source.includes('hostProvided'))return source;
  const ast=parse(source,{ecmaVersion:'latest',sourceType:'module'}),edits=[];
  let prefix='__walzUnusedHostResult';while(source.includes(prefix))prefix+='x';
  for(const node of nodes(ast)) {
    if(!['FunctionExpression','ArrowFunctionExpression'].includes(node.type)||!node.async||node.body.type!=='BlockStatement')continue;
    const body=node.body.body;
    if(body.some(n=>n.type==='ExpressionStatement'&&n.expression.type==='Literal'&&n.expression.value===marker))continue;
    if(!body.some(n=>n.type==='VariableDeclaration'&&n.declarations.some(d=>d.id.name==='hostProvided'&&d.init?.type==='Literal'&&d.init.value===true)))continue;
    const index=body.findIndex(n=>n.type==='IfStatement'&&n.test.type==='Identifier'&&n.test.name==='hostProvided'
      &&n.consequent.type==='BlockStatement'&&n.consequent.body.some(s=>s.type==='ReturnStatement'&&s.argument?.name==='ret')
      &&['resolve','exit'].every(name=>n.consequent.body.some(s=>s.type==='ExpressionStatement'&&s.expression.type==='CallExpression'
        &&s.expression.callee.type==='MemberExpression'&&s.expression.callee.object.name==='task'&&s.expression.callee.property.name===name)));
    if(index===-1)continue;
    let call=-1;
    for(let i=0;i<index;i++)if(body[i].type==='TryStatement'&&body[i].handler
      &&body[i].block.body.some(s=>s.type==='ExpressionStatement'&&s.expression.type==='AssignmentExpression'
        &&s.expression.left.name==='ret'&&s.expression.right.type==='AwaitExpression'))call=i;
    if(call===-1)throw new Error('native host-result compatibility no longer matches the pinned transpiler');
    // A rejected host task has a parent subtask. Returning its unsettled
    // completion promise after exit deadlocks the JSPI caller in 0.23.0.
    const handler=body[call].handler,tail=handler.body.body.at(-1);
    if(tail?.type==='ReturnStatement'&&tail.argument?.type==='CallExpression'&&tail.argument.callee.type==='MemberExpression'
      &&tail.argument.callee.object.name==='task'&&tail.argument.callee.property.name==='completionPromise'
      &&handler.param?.type==='Identifier')edits.push([tail.start,tail.end,`throw ${handler.param.name};`]);
    const allocators=body.slice(call+1,index).flatMap(nodes).filter(n=>['ArrowFunctionExpression','FunctionExpression'].includes(n.type)
      &&n.params.some(p=>p.type==='RestElement')&&nodes(n.body).some(c=>c.type==='CallExpression'&&c.callee.type==='Identifier'&&/^realloc\d+(?:Async)?$/.test(c.callee.name)));
    if(!allocators.length)continue;
    edits.push([node.body.start+1,node.body.start+1,`\n${JSON.stringify(marker)};\n`]);
    edits.push([body[call].end,body[call].end,`
const ${prefix}=new Map();
const ${prefix}Track=fn=>(...args)=>{
  const owned=args[0]===0||${prefix}.has(args[0]);
  const done=ptr=>{if(owned){if(args[0])${prefix}.delete(args[0]);if(ptr&&args[3])${prefix}.set(ptr,{fn,size:args[3],align:args[2]});}return ptr;};
  const result=fn(...args);return result&&typeof result.then==='function'?Promise.resolve(result).then(done):done(result);
};
`]);
    for(const allocator of allocators){edits.push([allocator.start,allocator.start,`${prefix}Track(`]);edits.push([allocator.end,allocator.end,')']);}
    edits.push([body[index].consequent.start+1,body[index].consequent.start+1,`
for(const [ptr,allocation]of ${prefix})await allocation.fn(ptr,allocation.size,allocation.align,0);
${prefix}.clear();
`]);
  }
  for(const[start,end,text]of edits.sort((a,b)=>b[0]-a[0]))source=source.slice(0,start)+text+source.slice(end);
  return source;
}
