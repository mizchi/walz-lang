import {readFile} from 'node:fs/promises';
const {instance}=await WebAssembly.instantiate(await readFile(process.argv[2]));
const actual={};for(const name of process.argv.slice(3))actual[name]=String(instance.exports[name]());
console.log(JSON.stringify(actual));
