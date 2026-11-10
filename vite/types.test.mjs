import {test} from 'node:test';
import assert from 'node:assert/strict';
import {declarations} from './types.mjs';

test('declarations reflect ABI direction, nested options, unit payloads and async assimilation', () => {
  const bindings = {
    bytes:{jsName:'bytes',params:['bytes'],result:'bytes',suspends:false},
    numbers:{jsName:'numbers',params:['(list i64)'],result:'(list i64)',suspends:false},
    choices:{jsName:'choices',params:['(option (option i32))'],result:'(result unit text)',suspends:false},
    record:{jsName:'record',params:['(record (my-value bool))'],result:'(flags first second)',suspends:false},
    cases:{jsName:'cases',params:['(variant (empty) (value i32))'],result:'(enum empty value)',suspends:false},
    future:{jsName:'future',params:['(future unit)'],result:'(future i64)',suspends:true},
    stream:{jsName:'stream',params:['(stream text)'],result:'(stream text)',suspends:false},
  };
  const code = declarations(bindings);
  for (const needle of ['Uint8Array', 'ReadonlyArray<bigint>', 'BigInt64Array', '"some"', '"none"',
    '"myValue"', '"empty"', 'PromiseLike<void>', 'Promise<Awaited<WalzFuture<bigint>>>', 'AsyncIterable<string>', 'WalzStream<string>']) {
    assert.ok(code.includes(needle), needle);
  }
  assert.ok(!code.includes(': any'));
  assert.throws(()=>declarations({bad:{jsName:'bad',params:[],result:'(not-a-type i32)',suspends:false}}), /Unsupported.*shape/);
});
