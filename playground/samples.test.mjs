import test from 'node:test';
import assert from 'node:assert/strict';
import { samples } from './samples.mjs';

test('the language tour covers the main features with explicit outcomes', () => {
  const required = new Set([
    'bindings', 'operators', 'scalars', 'native-wat', 'control-flow', 'loops',
    'recursion', 'structs', 'arrays', 'enums', 'option', 'result', 'tuples',
    'lists', 'closures', 'generics', 'angle-applications', 'type-bounds', 'modules', 'opaque',
    'effects', 'ownership', 'resources', 'futures', 'streams', 'cancellation',
    'strings', 'bytes', 'views', 'array-views', 'json', 'simd', 'tests', 'benchmarks', 'laws',
    'diagnostics',
    'component-imports', 'for-each', 'struct-patterns', 'field-updates',
    'generic-inference', 'user-macros', 'macro-inference', 'macro-bindings', 'macro-diagnostics',
  ]);
  const labels = new Set();
  for (const [id, sample] of Object.entries(samples)) {
    assert.ok(sample.category && sample.description, `${id}: category and explanation`);
    assert.ok(Array.isArray(sample.features) && sample.features.length, `${id}: features`);
    assert.ok(!labels.has(sample.label), `${id}: unique label`);
    labels.add(sample.label);
    assert.ok(['run', 'compile', 'diagnostic'].includes(sample.expectation?.kind), `${id}: outcome`);
    if (sample.expectation.kind === 'run') assert.equal(typeof sample.expectation.output, 'string', id);
    if (sample.expectation.kind === 'compile') assert.ok(sample.expectation.note && sample.expectation.imports.length, id);
    if (sample.expectation.kind === 'diagnostic') assert.match(sample.expectation.code, /^E_/, id);
    assert.match(sample.source.trim(), /^\(module\b/, `${id}: standalone module`);
    for (const feature of sample.features) required.delete(feature);
  }
  assert.deepEqual([...required], [], 'missing language features');
});
