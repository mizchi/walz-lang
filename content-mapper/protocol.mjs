/** LSP-style JSON-RPC frames. Lengths are bytes, not JavaScript code units. */
export async function* frames(stream) {
  let buffer=Buffer.alloc(0), length=null;
  for await (const chunk of stream) {
    buffer=Buffer.concat([buffer,chunk]);
    for (;;) {
      if (length === null) {
        const end=buffer.indexOf('\r\n\r\n');
        if (end < 0) { if (buffer.length>8192) throw new Error('Content Mapper header exceeds 8 KiB'); break; }
        if (end>8192) throw new Error('Content Mapper header exceeds 8 KiB');
        const matches=buffer.subarray(0,end).toString('ascii').split('\r\n').filter(line=>/^content-length:/i.test(line));
        const value=matches.length===1 && matches[0].slice(matches[0].indexOf(':')+1).trim();
        if (!value || !/^\d+$/.test(value) || Number(value)>16*1024*1024) throw new Error('Invalid Content-Length');
        length=Number(value); buffer=buffer.subarray(end+4);
      }
      if (buffer.length<length) break;
      yield buffer.subarray(0,length); buffer=buffer.subarray(length); length=null;
    }
  }
  if (buffer.length || length !== null) throw new Error('Truncated Content Mapper frame');
}

export function frame(message) {
  const body=Buffer.from(JSON.stringify(message));
  return Buffer.concat([Buffer.from(`Content-Length: ${body.length}\r\n\r\n`),body]);
}
