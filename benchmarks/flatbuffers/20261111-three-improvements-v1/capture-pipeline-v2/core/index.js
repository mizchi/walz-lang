import {instantiate as create} from './core.mjs';
import {bind} from './bindings.mjs';
export {TaskCancelledError,TaskTimeoutError} from './tasks.mjs';
export const instantiate=(options={})=>create(import.meta.url,"walz-core-js-v2",bind,options,[],[],null,null,null,null);
