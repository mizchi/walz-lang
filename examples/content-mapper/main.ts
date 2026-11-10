import {addOne} from './math.walz';

const answer: bigint = addOne(41n);
// @ts-expect-error Walz i64 parameters require bigint.
addOne(41);
console.log(answer);
