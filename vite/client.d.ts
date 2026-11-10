/// <reference types="vite/client" />
declare module '*.wlt?init' {
  import type {InstantiateOptions, Runtime} from '@mizchi/wlt-runtime/vite/types';
  export default function instantiate<API extends object = Record<string, (...args: unknown[]) => unknown>>(options?: InstantiateOptions): Promise<Runtime<API>>;
  export {instantiate};
  export class TaskCancelledError extends Error {}
  export class TaskTimeoutError extends TaskCancelledError { readonly timeoutMs: number; }
}
