export interface InstantiateOptions {
  imports?: Record<string, object>;
  eventLoop?: object;
  onTaskEvent?: (event: unknown) => void;
  atomicHostContract?: string;
  readonlyHostContract?: string;
  getCoreModule?: (name: string) => WebAssembly.Module | Promise<WebAssembly.Module>;
  instantiateCore?: (module: WebAssembly.Module, imports: WebAssembly.Imports) => WebAssembly.Instance | Promise<WebAssembly.Instance>;
}
export interface TaskOptions { signal?: AbortSignal; timeoutMs?: number; }
export interface Task<T> {
  readonly state: string;
  readonly result: Promise<T>;
  cancel(reason?: unknown): Promise<{status: 'returned' | 'failed' | 'cancelled'; started: boolean}>;
}
type Args<F> = F extends (...args: infer A) => unknown ? A : never;
// Synchronous Future results use resolveAsValue to avoid native Promise
// assimilation. TypeScript's Awaited cannot express that distinction safely.
type Result<F> = F extends (...args: never[]) => infer R
  ? R extends Promise<infer T> ? T : R extends PromiseLike<unknown> ? unknown : R : never;
export interface Runtime<API extends object> {
  readonly api: API;
  start<K extends keyof API & string>(name: K, args: Args<API[K]>, options?: TaskOptions): Task<Result<API[K]>>;
}
