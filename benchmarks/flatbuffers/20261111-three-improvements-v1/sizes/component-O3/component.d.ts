// world root:component/root
import type * as WalzRuntimeDelivery from './interfaces/walz-runtime-delivery.js'; // walz:runtime/delivery@0.1.0
import type * as WalzGeneratedApi from './interfaces/walz-generated-api.js'; // walz:generated/api@0.1.0
export interface ImportObject {
  "walz:runtime/delivery@0.1.0": typeof WalzRuntimeDelivery,
}
export interface Root {
  "walz:generated/api@0.1.0": typeof WalzGeneratedApi,
  api: typeof WalzGeneratedApi,
}

/**
* Options accepted by `instantiate`.
*/
export interface InstantiateOptions {
  /** Runtime overrides for this instance. */
  shim?: {
    /** Platform globals used instead of those on `globalThis`. */
    globals?: {
      /**
      * The WebAssembly implementation used by this instance,
      * including by the default `instantiateCore`. Defaults to
      * `globalThis.WebAssembly`.
      */
      WebAssembly?: typeof WebAssembly;
    };
  };
}


/**
* Instantiates this component with the provided imports and
* returns a map of all the exports of the component.
*
* This function is intended to be similar to the
* `WebAssembly.Instantiate` constructor. The second `imports`
* argument is the "import object" for wasm, except here it
* uses component-model-layer types instead of core wasm
* integers/numbers/etc.
*
* The first argument to this function, `getCoreModule`, is
* used to compile core wasm modules within the component.
* Components are composed of core wasm modules and this callback
* will be invoked per core wasm module. The caller of this
* function is responsible for reading the core wasm module
* identified by `path` and returning its compiled
* `WebAssembly.Module` object. This would use the
* `WebAssembly.Module` constructor on the web, for example.
*
* The optional `options` argument configures instantiation; see
* `InstantiateOptions`.
*/
export function instantiate(
getCoreModule: (path: string) => WebAssembly.Module,
imports: ImportObject,
instantiateCore?: (module: WebAssembly.Module, imports: Record<string, any>) => WebAssembly.Instance,
options?: InstantiateOptions
): Root;
export function instantiate(
getCoreModule: (path: string) => WebAssembly.Module | Promise<WebAssembly.Module>,
imports: ImportObject,
instantiateCore?: (module: WebAssembly.Module, imports: Record<string, any>) => WebAssembly.Instance | Promise<WebAssembly.Instance>,
options?: InstantiateOptions
): Root | Promise<Root>;

