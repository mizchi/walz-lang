/** @module Interface walz:generated/api@0.1.0 **/
export function combine(p0: number, p1: T7, p2: T7, p3: number): T7;
export function complement(p0: number, p1: T7): T7;
export function count(p0: number, p1: T7): number;
export function intersectionCount(p0: number, p1: T7, p2: T7): number;
export function indices(p0: number, p1: T7): T7;
export function roundtrip(p0: T7): T7;
export function get(p0: T7, p1: number): number;
export function sum(p0: T7): bigint;
export function sumWhere(p0: T7, p1: T7): bigint;
export function equal(p0: T7, p1: number): T7;
export function between(p0: T7, p1: number, p2: number): T7;
export function countEq(p0: T7, p1: number): number;
export function countBetween(p0: T7, p1: number, p2: number): number;
export function query(p0: T7, p1: T7, p2: number, p3: number, p4: number): T0;
export type WalzUnit = import('./walz-runtime-delivery.js').WalzUnit;
export type T0 = import('./walz-runtime-delivery.js').T0;
export type T7 = import('./walz-runtime-delivery.js').T7;
