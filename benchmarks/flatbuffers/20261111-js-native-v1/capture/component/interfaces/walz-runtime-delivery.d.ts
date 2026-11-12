/** @module Interface walz:runtime/delivery@0.1.0 **/
/**
 * # Variants
 * 
 * ## `"value"`
 */
export type WalzUnit = "value";
export type T7 = boolean;
export type T13 = string;
export interface T8 {
  count: number,
  label: T13,
}
export type T12 = Int32Array;
export type T10 = Uint8Array;
export type T14 = T8 | undefined;
export interface T9 {
  id: bigint,
  name: T13,
  values: T12,
  payload: T10,
  enabled: T7,
  weight: number,
  child?: T8,
}
export type T11 = Result<T9, number>;
export type T15 = T13 | undefined;
export type Result<T, E> = { tag: 'ok', val: T } | { tag: 'err', val: E };
