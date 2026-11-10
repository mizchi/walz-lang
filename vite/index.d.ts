import type {Plugin} from 'vite';

export interface WalzOptions {
  /** Installed walz CLI, or executable followed by prefix arguments. Defaults to WALZ or walz. */
  compiler?: string | readonly [string, ...string[]];
  /** Compiler timeout in milliseconds; defaults to 60000. */
  timeoutMs?: number;
  /** Binding policy path, resolved from Vite's root. */
  bindings?: string;
  /** Map Walz host namespaces to ESM providers for direct imports. Relative paths use Vite's root. */
  imports?: Record<string, string>;
  /** Write checked named-export declarations beside sources as *.d.walz.ts. Defaults to false. */
  dts?: boolean;
}
export default function walz(options?: WalzOptions): Plugin;
export type {InstantiateOptions, TaskOptions, Task, Runtime} from './runtime.js';
