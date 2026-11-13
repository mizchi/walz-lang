import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

export class NativePeer {
  constructor(binary) {
    this.pending = [];
    this.child = spawn(binary, [], { stdio: ["pipe", "pipe", "pipe"] });
    this.stderr = "";
    this.child.stderr.on("data", data => { this.stderr += data; });
    this.lines = createInterface({ input: this.child.stdout });
    this.lines.on("line", line => {
      const waiter = this.pending.shift();
      if (!waiter) return;
      try {
        const response = JSON.parse(line);
        if (response.error) waiter.reject(new Error(response.error));
        else waiter.resolve(response.ok);
      } catch (error) { waiter.reject(error); }
    });
    this.exited = new Promise(resolve => {
      const fail = error => {
        this.failure = error;
        for (const p of this.pending.splice(0)) p.reject(error);
      };
      this.child.on("error", error => { fail(error); resolve(-1); });
      this.child.on("close", code => {
        fail(new Error(`native peer exited (${code}): ${this.stderr}`)); resolve(code);
      });
    });
  }
  request(value) {
    if (this.failure) return Promise.reject(this.failure);
    return new Promise((resolve, reject) => {
      this.pending.push({ resolve, reject });
      this.child.stdin.write(JSON.stringify(value) + "\n");
    });
  }
  async close() {
    this.child.stdin.end();
    const code = await this.exited;
    if (code !== 0) throw this.failure;
  }
}
