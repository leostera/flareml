import {Schema} from 'effect';
import {Lines} from '../lib/process';
import type {Task} from '../tasks/specs';
import {canonical,initial,linearizations,type Json,type Observation,type State} from './reference';
import type {Case} from './cases';
const text=Schema.String;
const Action=Schema.Union(
 Schema.Struct({kind:Schema.Literal('get'),key:text,token:text}),
 Schema.Struct({kind:Schema.Literal('cas'),key:text,token:text,expected:Schema.Unknown,value:Schema.Unknown}),
 Schema.Struct({kind:Schema.Literal('reply'),id:text,result:Schema.Unknown}),
);
const decode=Schema.decodeUnknownSync(Schema.Array(Action),{onExcessProperty:'error'});
export function validStorageValue(value:unknown):boolean {
 const encoded=JSON.stringify(value);
 return value!==null&&encoded!==undefined&&Buffer.byteLength(encoded,'utf8')<=65536;
}
export type CaseResult={name:string;passed:boolean;error?:string;steps:number;history:unknown[]};
export async function scenario(task:Task,c:Case,command:string[],cwd:string):Promise<CaseResult>{
 let random=c.seed>>>0;const next=()=>{random=(Math.imul(random,1664525)+1013904223)>>>0;return random;};
 const workers:[Lines,Lines]=[new Lines(command,cwd),new Lines(command,cwd)];
 const kv=new Map<string,unknown>();const pending:{worker:number;kind:'event'|'storage';value:any}[]=[];
 const outstanding:[Set<string>,Set<string>]=[new Set<string>(),new Set<string>()];const requests=new Map<string,{observation:Observation;worker:number;replied:boolean;lostAck:boolean}>();
 const history:unknown[]=[];let tick=0,serial=0,steps=0;let states:State[]=[initial(task,c.config)];
 function actions(worker:number,raw:unknown){
  for(const action of decode(raw)){
   history.push({tick:++tick,worker,action});
   if(action.kind==='reply'){
    const r=requests.get(action.id);if(!r||r.worker!==worker||r.replied)throw new Error('unknown, misrouted, or duplicate client reply');
    r.replied=true;r.observation.result=action.result as Json;r.observation.end=tick;
    if(r.lostAck)history.push({tick,acknowledgement:'lost-to-client',id:action.id});
   }else{
    if(outstanding[worker]!.has(action.token))throw new Error('storage token reused while outstanding');
    if(action.key.length>512||action.token.length>512)throw new Error('storage key/token too long');
    if(action.kind==='cas'&&!validStorageValue(action.value))throw new Error('invalid storage value');
    outstanding[worker]!.add(action.token);pending.push({worker,kind:'storage',value:action});
   }
  }
 }
 async function init(worker:number){const result=await workers[worker]!.send({kind:'init',config:c.config,worker});if(canonical(result)!=='[]')throw new Error('init must return []');}
 try{
  await init(0);await init(1);
  for(const [batchIndex,batch] of c.batches.entries()){
   const ids=batch.map(client=>{const id=`r${serial++}`;requests.set(id,{worker:client.worker,replied:false,lostAck:client.lostAck??false,observation:{request:client.request,result:null,start:++tick,end:Infinity}});pending.push({worker:client.worker,kind:'event',value:{kind:'request',id,request:client.request}});return id;});
   while(ids.some(id=>!requests.get(id)!.replied)){
    if(++steps>2000)throw new Error('2000-step liveness budget exceeded');
    if(!pending.length)throw new Error('no pending work but clients have no reply');
    const item=pending.splice(next()%pending.length,1)[0]!;
    if(item.kind==='storage'){
     const action=item.value;let value=kv.has(action.key)?kv.get(action.key):null;let ok: boolean|undefined;
     if(action.kind==='cas'){ok=canonical(value)===canonical(action.expected);if(ok){value=structuredClone(action.value);kv.set(action.key,value);}}
     pending.push({worker:item.worker,kind:'event',value:{kind:'result',token:action.token,value,...(ok===undefined?{}:{ok})}});
     history.push({tick:++tick,storage:action,ok,value});
    }else{
     if(item.value.kind==='result')outstanding[item.worker]!.delete(item.value.token);
     history.push({tick:++tick,worker:item.worker,event:item.value});
     actions(item.worker,await workers[item.worker]!.send(item.value));
    }
   }
   states=linearizations(task,states,ids.map(id=>requests.get(id)!.observation));
   if(!states.length)throw new Error(`batch ${batchIndex}: no legal linearization matches replies`);
   if(c.restartAfter?.includes(batchIndex)){
    // Acknowledged work must already be durable. Discard uncommitted in-flight
    // requests/completions and process-local state, but preserve committed KV.
    pending.length=0;outstanding[0].clear();outstanding[1].clear();
    await workers[0].close();await workers[1].close();workers[0]=new Lines(command,cwd);workers[1]=new Lines(command,cwd);
    history.push({tick:++tick,restart:[0,1]});await init(0);await init(1);
   }
  }
  // Drain post-reply work to detect duplicate replies and nonterminating cleanup.
  while(pending.length){if(++steps>2000)throw new Error('post-reply liveness budget exceeded');const item=pending.shift()!;
   if(item.kind==='storage'){const a=item.value;let value=kv.has(a.key)?kv.get(a.key):null;let ok:boolean|undefined;if(a.kind==='cas'){ok=canonical(value)===canonical(a.expected);if(ok){value=structuredClone(a.value);kv.set(a.key,value);}}pending.push({worker:item.worker,kind:'event',value:{kind:'result',token:a.token,value,...(ok===undefined?{}:{ok})}});}
   else{if(item.value.kind==='result')outstanding[item.worker]!.delete(item.value.token);actions(item.worker,await workers[item.worker]!.send(item.value));}
  }
  return {name:c.name,passed:true,steps,history};
 }catch(error){return {name:c.name,passed:false,steps,error:String(error),history};}
 finally{await Promise.all(workers.map(w=>w.close()));}
}
