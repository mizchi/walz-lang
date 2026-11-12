/** @module Interface walz:runtime/delivery@0.1.0 **/
/**
 * # Variants
 * 
 * ## `"value"`
 */
export type WalzUnit = "value";
export type T11 = boolean;
export type T18 = string;
export type T19 = number | undefined;
export interface T0 {
  id: bigint,
  name: T18,
  count: number,
  sum: bigint,
  size: number,
  enabled: T11,
  weight: number,
  child?: number,
}
export interface T12 {
  count: number,
  label: T18,
}
export type T27 = Int32Array;
export type T17 = Uint8Array;
export type T28 = T12 | undefined;
export interface T13 {
  id: bigint,
  name: T18,
  values: T27,
  payload: T17,
  enabled: T11,
  weight: number,
  child?: T12,
}
export type T20 = Result<T0, number>;
export type T21 = Result<T17, number>;
export type T22 = Array<T17>;
export type T23 = Array<T20>;
export type T24 = Array<T21>;
export type T25 = Result<T13, number>;
export type T26 = T18 | undefined;
export type Result<T, E> = { tag: 'ok', val: T } | { tag: 'err', val: E };
