// Node provider for explicitly selected files. No guest-controlled path lookup.
import {open} from 'node:fs/promises';
import {createWasiFileSystem} from './wasi-filesystem.mjs';

function selected(path,write) {
  return {path:'selected',directory:{async openAt(_flags,_path,_open,access){
    if(Boolean(access.write)!==write)throw 'not-permitted';
    const file=await open(path,write?'w':'r');let closed=false;
    return {
      readViaStream(offset){
        const completion=Promise.withResolvers();let finished=false,reading=false,position=Number(offset),at=0,length=0;
        const buffer=new Uint8Array(16384);
        const stream={
          [Symbol.asyncIterator](){return this;},
          async readMany(max){
            if(!Number.isInteger(max)||max<1||max>65536)throw new RangeError('invalid batch count');
            if(reading)throw new TypeError('concurrent file read');
            if(finished||closed)return {items:[],done:true};
            reading=true;
            try{
              if(at===length){const result=await file.read(buffer,0,buffer.length,position);length=result.bytesRead;at=0;position+=length;}
              if(!length){finished=true;completion.resolve({tag:'ok',val:undefined});return {items:[],done:true};}
              const end=Math.min(length,at+max),items=buffer.slice(at,end);at=end;
              return {items,done:false};
            }catch(error){finished=true;completion.resolve({tag:'err',val:error.code==='ENOENT'?'no-entry':'io'});return {items:[],done:true};}
            finally{reading=false;}
          },
          async next(){const {items}=await this.readMany(1);return items.length?{value:items[0],done:false}:{done:true};},
          async return(){if(!finished){finished=true;completion.resolve({tag:'err',val:'interrupted'});}at=length;return {done:true};},
        };
        return [stream,completion.promise];
      },
      async writeViaStream(data,offset){
        let position=Number(offset),length=0;const buffer=new Uint8Array(16384);
        const flush=async()=>{let done=0;while(done<length){const {bytesWritten}=await file.write(buffer,done,length-done,position+done);if(!bytesWritten)throw new Error('short write');done+=bytesWritten;}position+=length;length=0;};
        const iterator=data[Symbol.asyncIterator]();
        for(;;){
          const batch=iterator.readMany?await iterator.readMany(16384):await Promise.resolve(iterator.next()).then(r=>({items:r.done?[]:[r.value],done:r.done}));
          for(const byte of batch.items){if(!Number.isInteger(byte)||byte<0||byte>255)throw 'invalid';buffer[length++]=byte;if(length===buffer.length)await flush();}
          if(batch.done)break;
        }
        if(length)await flush();return {tag:'ok',val:undefined};
      },
      async [Symbol.asyncDispose](){if(!closed){closed=true;await file.close();}},
    };
  }}};
}
export function createNodeFileSystem({readFiles={},writeFiles={}}={}) {
  return createWasiFileSystem({
    readFiles:Object.fromEntries(Object.entries(readFiles).map(([name,path])=>[name,selected(path,false)])),
    writeFiles:Object.fromEntries(Object.entries(writeFiles).map(([name,path])=>[name,selected(path,true)])),
  });
}
