import {instantiate as create} from './runtime.mjs';
import {bind} from './bindings.mjs';
const manifest={"abi":"walz-js-native-experiment-v1","jsStrings":false,"maximumTransferBytes":16777216,"note":"Experimental shared-value JS ABI, not Canonical ABI; no start/cancellation/host imports.","synchronous":true};
export const instantiate=(options={})=>create(import.meta.url,bind,manifest,options);
