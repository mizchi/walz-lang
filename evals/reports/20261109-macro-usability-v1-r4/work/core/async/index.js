export {withTaskScope} from './scopes.mjs';
import {install} from './readables.mjs';
import {instantiate as create} from './core.mjs';
import {bind} from './bindings.mjs';
export {TaskCancelledError,TaskTimeoutError} from './tasks.mjs';
export const instantiate=(options={})=>create(import.meta.url,"wlt-core-js-v2",bind,options,[{"capability":"clock","effects":{"host":true,"suspends":false},"jsName":"deferI64","module":"wlt:host/event-loop@0.1.0","name":"defer-i64","params":["i32","i64"],"resource":null,"result":"(future i64)"}],[],null,null,install,null);
