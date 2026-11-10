import {addOne} from './math.wlt';

const answer: bigint = addOne(41n);
// @ts-expect-error WLT i64 parameters require bigint.
addOne(41);
console.log(answer);
