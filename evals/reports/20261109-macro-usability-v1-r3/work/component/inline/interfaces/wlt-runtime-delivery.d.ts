/** @module Interface wlt:runtime/delivery@0.1.0 **/
/**
 * # Variants
 * 
 * ## `"value"`
 */
export type WltUnit = "value";
export type T19 = Uint8Array;
export type T20 = string;
export type T21 = T20 | undefined;
export type T22 = bigint | undefined;
export type T23 = Result<number, number>;
export type T24 = [T20, number];
export type T25 = Result<T24, number>;
export type T34 = [T20, bigint];
export type T35 = Array<T34>;
export type T36 = Array<T20>;
export type Result<T, E> = { tag: 'ok', val: T } | { tag: 'err', val: E };
