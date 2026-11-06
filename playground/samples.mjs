import { basics } from './samples/basics.mjs';
import { data } from './samples/data.mjs';
import { functions } from './samples/functions.mjs';
import { standard } from './samples/standard.mjs';
import { asyncSamples } from './samples/async.mjs';
import { checks } from './samples/checks.mjs';

// Standalone lessons with executable outcomes. Keep compiler and host limits
// explicit rather than presenting a host-dependent example as a runnable one.
export const samples = { ...basics, ...data, ...functions, ...standard, ...asyncSamples, ...checks };
