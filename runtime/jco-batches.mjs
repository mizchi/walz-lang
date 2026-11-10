// Optional readMany(max) transport, isolated to the pinned jco stream glue.
export function batchStreams(source){
  if(!source.includes('class PendingValueQueue'))return source;
  const next=/(async next\(\) \{\n)([ \t]+)(_debugLog\('\[Stream#next\(\)\]'\);)/;
  if(!next.test(source))throw new Error('jco batch Stream wrapper no longer matches');
  source=source.replace(next,(_,head,indent,tail)=>`async readMany(max){const r=await this.read({count:max,walzBatch:true});return {items:r.done?[]:Array.from(r.value),done:!!r.done};}\n${indent.slice(0,-2)}${head}${indent}${tail}`);
  const edits=[
    ['return { count, rejectLength, dropAfterCopy };','return { count, rejectLength, dropAfterCopy, walzBatch:opts?.walzBatch===true };'],
    ['typedArray === undefined ? count === 1 ? values[0] : values : new typedArray(values)','typedArray === undefined ? count === 1 && !opts?.walzBatch ? values[0] : values : new typedArray(values)'],
    ['async readSource() {','async readSource(max=1) {'],
    ['() => this.#readFn()','() => this.#readFn(max)'],
    ['const appended = res.done ? 0 : this.appendReadValue(res.value);',"const appended=res.value?.[Symbol.for('walz.stream.batch')]?this.push(res.value.items):res.done?0:this.appendReadValue(res.value);"],
    ['pendingValues.readSource()','pendingValues.readSource(count||1)'],
    ['readFn = () => asyncIterator.next();',`readFn = asyncIterator.readMany?async max=>{const r=await asyncIterator.readMany(max);if(!r||!r.items||!Number.isInteger(r.items.length)||r.items.length>max||(!r.items.length&&!r.done))throw new TypeError('invalid stream batch');return {done:!!r.done,value:{[Symbol.for('walz.stream.batch')]:true,items:r.items}};}:()=>asyncIterator.next();`],
  ];
  for(const[from,to]of edits){if(!source.includes(from))throw new Error('jco batch stream patch no longer matches: '+from);source=source.replaceAll(from,to);}
  return source;
}
