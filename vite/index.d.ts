import type {Plugin} from 'vite';

export interface WltOptions {
  /** Installed wlt CLI, or executable followed by prefix arguments. Defaults to WLT or wlt. */
  compiler?: string | readonly [string, ...string[]];
  /** Compiler timeout in milliseconds; defaults to 60000. */
  timeoutMs?: number;
  /** Binding policy path, resolved from Vite's root. */
  bindings?: string;
  /** Map WLT host namespaces to ESM providers for direct imports. Relative paths use Vite's root. */
  imports?: Record<string, string>;
  /** Write checked named-export declarations beside sources as *.d.wlt.ts. Defaults to false. */
  dts?: boolean;
}
export default function wlt(options?: WltOptions): Plugin;
export type {InstantiateOptions, TaskOptions, Task, Runtime} from './runtime.js';
