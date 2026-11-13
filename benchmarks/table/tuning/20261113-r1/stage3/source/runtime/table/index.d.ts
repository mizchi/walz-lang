export type Cell = number | string | boolean | null | undefined;
export type Row = Record<string, Cell>;
export type Column = readonly (number | null | undefined)[] | readonly (string | null | undefined)[] |
  readonly (boolean | null | undefined)[] | Int8Array | Uint8Array | Uint8ClampedArray | Int16Array |
  Uint16Array | Int32Array | Uint32Array | Float32Array | Float64Array;
export type Columns = Readonly<Record<string, Column>>;
type InputRow<C extends Columns> = { [K in keyof C & string]: C[K] extends ArrayLike<infer V> ? Extract<V, Cell> : never };
declare const expressionBrand: unique symbol;
declare const descendingBrand: unique symbol;
export interface OpExpression<T extends Cell = Cell> { readonly [expressionBrand]: T }
export type Expression<T extends Row = Row> = ((row: T) => Cell) | Exclude<Cell, string>;
export type AggregateExpression<T extends Row = Row> = Expression<T> | OpExpression | ((row: T) => Cell | OpExpression);
export interface Descending { readonly [descendingBrand]: true }
export type OrderExpression<T extends Row = Row> = string | Expression<T> | Descending;
type Value<E> = E extends OpExpression<infer V> ? V : Extract<E, Cell>;
type Result<E> = E extends (...args: never[]) => infer V ? Value<V> : Value<E>;
type Results<D> = { [K in keyof D]: Result<D[K]> };
export interface JoinOptions { suffix?: readonly [string, string] }
export type JoinKeys = string | readonly [string | readonly string[]] | readonly [string | readonly string[], string | readonly string[]];
export interface Table<T extends Row = Row> {
  numRows(): number;
  numCols(): number;
  columnNames(): string[];
  isGrouped(): boolean;
  get<K extends keyof T & string>(name: K, row?: number): T[K] | undefined;
  array<K extends keyof T & string>(name: K): T[K][];
  objects(): T[];
  select(): Table<{}>;
  select<K extends keyof T & string>(...names: (K | readonly K[])[]): Table<Pick<T, K>>;
  filter(predicate: Expression<T>): Table<T>;
  derive<D extends Record<string, Expression<T>>>(specs: D): Table<Omit<T, keyof D> & Results<D>>;
  groupby(...keys: (string | readonly string[] | Record<string, Expression<T> | string>)[]): Table<T>;
  ungroup(): Table<T>;
  orderby(...keys: (OrderExpression<T> | readonly OrderExpression<T>[])[]): Table<T>;
  slice(start?: number, end?: number): Table<T>;
  rollup<D extends Record<string, AggregateExpression<T>>>(specs?: D): Table<Row & Results<D>>;
  count(options?: { as?: string }): Table;
  join(other: Table, on?: JoinKeys, values?: null, options?: JoinOptions): Table;
  join_left(other: Table, on?: JoinKeys, values?: null, options?: JoinOptions): Table;
  join_right(other: Table, on?: JoinKeys, values?: null, options?: JoinOptions): Table;
  join_full(other: Table, on?: JoinKeys, values?: null, options?: JoinOptions): Table;
  semijoin(other: Table, on?: JoinKeys): Table;
  antijoin(other: Table, on?: JoinKeys): Table;
}
export const Table: { readonly prototype: Table; [Symbol.hasInstance](value: unknown): boolean };
type NumericAggregate = <T extends Row>(argument?: string | Expression<T>) => OpExpression<number | undefined>;
type CountAggregate = <T extends Row>(argument?: string | Expression<T>) => OpExpression<number>;
export const op: Readonly<{
  count: CountAggregate; valid: CountAggregate; invalid: CountAggregate; distinct: CountAggregate;
  sum: NumericAggregate; mean: NumericAggregate; average: NumericAggregate;
  min: <T extends Row>(argument?: string | Expression<T>) => OpExpression;
  max: <T extends Row>(argument?: string | Expression<T>) => OpExpression;
}>;
export function desc<T extends Row>(value: OrderExpression<T>): Descending;
export interface TableNamespace {
  table<C extends Columns>(data: C): Table<InputRow<C>>;
  from<T extends Row>(data: readonly T[]): Table<T>;
  from<C extends Columns>(data: C): Table<InputRow<C>>;
  readonly op: typeof op;
  readonly desc: typeof desc;
}
/** Instantiate one GC domain; tables from different domains must be copied explicitly. */
export interface TableOptions { filter?: "simd" | "scalar" | "packed-scalar"; io?: "bulk" | "scalar" }
export function instantiate(base: string | URL, options?: TableOptions): Promise<Readonly<TableNamespace>>;
