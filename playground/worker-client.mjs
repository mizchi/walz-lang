// A worker owns each pending operation. Timeout/cancellation destroys that
// worker, including a trapped compiler instance or an infinite Wasm guest.
export class WorkerClient {
  constructor(url, { timeout, reuse = true }) {
    this.url = url;
    this.timeout = timeout;
    this.reuse = reuse;
    this.nextId = 0;
  }

  run(payload, transfer = []) {
    if (this.pending) throw new Error('Wait for the current operation to finish');
    if (!this.worker) {
      this.worker = new Worker(this.url, { type: 'module' });
      this.worker.onmessage = ({ data }) => {
        if (data.id !== this.pending?.id) return;
        const pending = this.pending;
        this.pending = undefined;
        clearTimeout(pending.timer);
        if (!this.reuse) this.cancel();
        if (data.error) pending.reject(new Error(data.error));
        else pending.resolve(data.result);
      };
      this.worker.onerror = event => {
        event.preventDefault();
        this.cancel(new Error(event.message || 'Could not start the worker'));
      };
      this.worker.onmessageerror = () => this.cancel(new Error('Could not read the worker response'));
    }
    return new Promise((resolve, reject) => {
      const id = ++this.nextId;
      const timer = setTimeout(() => this.cancel(new Error('Time limit exceeded; operation stopped')), this.timeout);
      this.pending = { id, resolve, reject, timer };
      try { this.worker.postMessage({ id, ...payload }, transfer); }
      catch (error) { this.cancel(error); }
    });
  }

  cancel(error = new Error('Stopped')) {
    this.worker?.terminate();
    this.worker = undefined;
    if (this.pending) {
      clearTimeout(this.pending.timer);
      this.pending.reject(error);
      this.pending = undefined;
    }
  }
}
