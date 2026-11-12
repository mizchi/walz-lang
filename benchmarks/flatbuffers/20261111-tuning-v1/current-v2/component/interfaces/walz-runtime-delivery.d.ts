/** @module Interface walz:runtime/delivery@0.1.0 **/
/**
 * # Variants
 * 
 * ## `"value"`
 */
export type WalzUnit = "value";
export type T6 = boolean;
export type T12 = string;
export interface T7 {
  count: number,
  label: T12,
}
export type T11 = Int32Array;
export type T9 = Uint8Array;
export type T13 = T7 | undefined;
export interface T8 {
  id: bigint,
  name: T12,
  values: T11,
  payload: T9,
  enabled: T6,
  weight: number,
  child?: T7,
}
export type T10 = Result<T8, number>;
export type T14 = T12 | undefined;
export type Result<T, E> = { tag: 'ok', val: T } | { tag: 'err', val: E };
