import {main, echo, bytes, addOne, wait, trap} from './program.wlt';
import type {WltExports} from './program.wlt';
import init from './program.wlt?init';

function show(id: string, value: unknown) {
  document.getElementById(id)!.textContent = String(value);
}
show('answer', main());
show('text', echo('Hello, 世界 🌍'));
show('bytes', [...bytes(new Uint8Array([0, 255, 42]))].join(', '));
show('wide', addOne(41n));
show('future', await wait(Promise.resolve(42)));
document.getElementById('new-instances')!.onclick = async () => {
  const first = await init<WltExports>();
  const second = await init<WltExports>();
  show('instances', first.api.main() + second.api.main());
};
document.getElementById('trap-button')!.onclick = () => {
  try { trap(); } catch (error) { show('trap', (error as Error).name); }
};
