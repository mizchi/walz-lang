import test from 'node:test';
import assert from 'node:assert/strict';
import { measure } from './bench-runner.mjs';

test('fixed batches exclude calibration, preserve sample order and divide by operations', () => {
  let clock = 0;
  const calls = [];
  const result = measure(n => { calls.push(n); clock += n * 0.25; }, {
    iterations: 4, warmup_ms: 2, samples: 3,
  }, () => clock);
  assert.deepEqual(calls, [4, 4, 4, 4, 4]);
  assert.deepEqual(result, { iterations: 4, samples_ns: [250000, 250000, 250000] });
});

test('adaptive batches reach the target and calibration is excluded from samples', () => {
  let clock = 0;
  const calls = [];
  const result = measure(n => { calls.push(n); clock += n; }, {
    iterations: null, warmup_ms: 0, samples: 2, sample_ms: 5,
  }, () => clock);
  assert.deepEqual(calls, [1, 2, 4, 8, 8, 8]);
  assert.deepEqual(result, { iterations: 8, samples_ns: [1e6, 1e6] });
});

test('calibration caps counts without wrapping and traps escape immediately', () => {
  const calls = [];
  const result = measure(n => calls.push(n), {
    iterations: null, warmup_ms: 0, samples: 1, sample_ms: 1,
  }, () => 0);
  assert.equal(result.iterations, 0x7fffffff);
  assert.equal(calls.at(-1), 0x7fffffff);
  assert.ok(calls.every(n => n > 0 && n <= 0x7fffffff));
  assert.throws(() => measure(() => { throw new Error('trap'); }, {
    iterations: 1, warmup_ms: 0, samples: 1,
  }, () => 0), /trap/);
});
