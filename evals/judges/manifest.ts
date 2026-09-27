import {Lines} from '../lib/process';
import type {DistributedResult} from './checkpoint';
export const manifestCases=[1,17,1729,64007].flatMap(seed=>['uncertain-publication','inflight-restart','focused-reader-gap'].map(variant=>({name:`${variant}/seed-${seed}`,seed,variant})));
export async function manifestScenario(c:typeof manifestCases[number],command:string[],cwd:string):Promise<DistributedResult>{
 let steps=0,tick=0,serial=0,random=c.seed>>>0;const history:any[]=[];const next=()=>random=(Math.imul(random,1664525)+1013904223)>>>0;
 const nodes=Array.from({length:2},()=>({process:new Lines(command,cwd),inc:0,tokens:new Set<string>()}));
 const heads=new Map<string,number|null>([['a',null],['b',null]]),blobs=new Map<string,string>(),payloads=new Map<string,string>();
 const headHistory:{tick:number;artifact:string;version:number|null}[]=[{tick:0,artifact:'a',version:null},{tick:0,artifact:'b',version:null}];
 const requests=new Map<string,{node:number;request:any;start:number;end?:number;abandoned?:boolean}>();const queue:any[]=[];const mutations=new Map<string,number>();
 const key=(a:string,v:number)=>`${a}/${v}`;
 function record(e:any){history.push({tick:++tick,...e});}
 function safety(){for(const [artifact,v] of heads)if(v!==null&&!blobs.has(key(artifact,v)))throw new Error('head references missing blob');}
 function possible(artifact:string,start:number){const events=headHistory.filter(e=>e.artifact===artifact);const prior=events.filter(e=>e.tick<=start).at(-1)!;return [prior,...events.filter(e=>e.tick>start)].map(e=>e.version);}
 async function input(i:number,e:any){const n=nodes[i]!;record({node:i,input:e});const actions=await n.process.send(e);if(!Array.isArray(actions)||actions.length>64)throw new Error('expected bounded action array');if(e.kind==='init'&&actions.length)throw new Error('init must return []');
  for(const a of actions){record({node:i,action:a});if(a.kind==='reply'){
   if(Object.keys(a).some(k=>!['kind','id','result'].includes(k)))throw new Error('extra reply fields');
   const r=requests.get(a.id);if(!r||r.node!==i||r.end!==undefined||r.abandoned)throw new Error('invalid client reply');const result=a.result;const req=r.request;const seen=possible(req.artifact,r.start);
   if(req.op==='publish'){
    if(JSON.stringify(Object.keys(result).sort())!=='["status"]'||!(result.status==='published'?seen.includes(req.version):result.status==='superseded'&&seen.some(v=>v!==null&&v>req.version)))throw new Error('publish reply has no legal linearization');
   }else if(req.op==='read'){
    if(result.status==='missing'){if(Object.keys(result).length!==1||!seen.includes(null))throw new Error('invalid missing read');}
    else if(result.status!=='ok'||Object.keys(result).length!==3||!seen.includes(result.version)||payloads.get(key(req.artifact,result.version))!==result.payload)throw new Error('incoherent/stale read');
   }else if(JSON.stringify(result)!=='{"status":"ok"}')throw new Error('invalid collect reply');
   r.end=tick;
  }else{
   const base=['kind','artifact','token'];const fields=a.kind==='head-cas'?[...base,'expected','version']:a.kind==='blob-put'?[...base,'version','payload']:['blob-read','blob-delete'].includes(a.kind)?[...base,'version']:base;
   if(!['head-read','head-cas','blob-put','blob-read','blob-list','blob-delete'].includes(a.kind)||Object.keys(a).some(k=>!fields.includes(k))||!heads.has(a.artifact)||typeof a.token!=='string'||a.token.length>512||n.tokens.has(a.token))throw new Error('invalid storage action/token');
   if('version'in a&&(!Number.isInteger(a.version)||a.version<1||a.version>32))throw new Error('invalid version');
   if(a.kind==='head-cas'&&a.expected!==null&&!Number.isInteger(a.expected))throw new Error('invalid CAS expectation');
   if(a.kind==='blob-put'&&payloads.get(key(a.artifact,a.version))!==a.payload)throw new Error('incorrect/unrequested immutable payload');
   n.tokens.add(a.token);queue.push({kind:'storage',node:i,inc:n.inc,action:a});
  }}
 }
 async function step(allowed:(item:any)=>boolean=()=>true){if(++steps>5000)throw new Error('5000-event liveness bound');const eligible=queue.map((item,i)=>({item,i})).filter(x=>allowed(x.item));if(!eligible.length)throw new Error('no eligible pending work');const item=queue.splice(eligible[next()%eligible.length]!.i,1)[0]!;const n=nodes[item.node]!;
  if(item.kind==='storage'){
   const a=item.action;const old=heads.get(a.artifact)!;let value:any=null;let status='ok';
   const mut=['head-cas','blob-put','blob-delete'].includes(a.kind);let apply=true;
   if(mut){const count=(mutations.get(a.kind)??0)+1;mutations.set(a.kind,count);if(count<=3){status='unknown';apply=a.kind==='head-cas'||count%2===1;}}
   if(a.kind==='head-read')value=old;
   else if(a.kind==='blob-read')value=blobs.get(key(a.artifact,a.version))??null;
   else if(a.kind==='blob-list')value=[...blobs.keys()].filter(k=>k.startsWith(a.artifact+'/')).map(k=>Number(k.split('/')[1])).sort((a,b)=>a-b);
   else if(a.kind==='blob-put'&&apply)blobs.set(key(a.artifact,a.version),a.payload);
   else if(a.kind==='blob-delete'&&apply)blobs.delete(key(a.artifact,a.version));
   else if(a.kind==='head-cas'){
    const swapped=old===a.expected;if(swapped&&apply){if(old!==null&&a.version<old)throw new Error('head regressed');heads.set(a.artifact,a.version);headHistory.push({tick:tick+1,artifact:a.artifact,version:a.version});}
    value={swapped,current:heads.get(a.artifact)};
   }
   record({storage:a,status,applied:apply,value});safety();queue.push({kind:'event',node:item.node,inc:item.inc,event:{kind:'result',token:a.token,status,value:status==='unknown'?null:value}});
  }else if(item.inc===n.inc){if(item.event.kind==='result')n.tokens.delete(item.event.token);await input(item.node,item.event);}
 }
 function submit(node:number,request:any){const id=`r${serial++}`;if(request.op==='publish')payloads.set(key(request.artifact,request.version),request.payload);requests.set(id,{node,request,start:++tick});queue.push({kind:'event',node,inc:nodes[node]!.inc,event:{kind:'request',id,request}});return id;}
 async function batch(items:{node:number;request:any}[],heldNode?:number){const allowed=(item:any)=>item.node!==heldNode;const ids=items.map(x=>submit(x.node,x.request));while(ids.some(id=>requests.get(id)!.end===undefined)){if(!queue.some(allowed))throw new Error('request stalled');await step(allowed);}while(queue.some(allowed))await step(allowed);}
 const pub=(node:number,version:number,artifact='a')=>({node,request:{op:'publish',artifact,version,payload:`${artifact}/value-${version}`}});
 const read=(node:number,artifact='a')=>({node,request:{op:'read',artifact}});
 const collect=(node:number,artifact='a')=>({node,request:{op:'collect',artifact}});
 try{
  for(let i=0;i<2;i++)await input(i,{kind:'init',node:i});await batch([read(0),read(1)]);
  if(c.variant==='focused-reader-gap'){
   await batch([pub(1,1)]);
   const id=submit(0,read(0).request);
   // Pause before executing the reader's blob fetch, then publish and reclaim
   // that old version on the other independent frontend.
   while(requests.get(id)!.end===undefined&&!queue.some(x=>x.node===0&&x.kind==='storage'&&x.action.kind==='blob-read'&&x.action.version===1))await step(x=>x.node===0);
   await batch([pub(1,2)],0);await batch([collect(1)],0);
   while(requests.get(id)!.end===undefined)await step();while(queue.length)await step();
  }else if(c.variant==='inflight-restart'){
   const id=submit(0,pub(0,1).request);for(let k=0;k<5&&queue.length;k++)await step();requests.get(id)!.abandoned=true;
   const n=nodes[0]!;await n.process.close();n.inc++;n.tokens.clear();n.process=new Lines(command,cwd);record({restart:0});await input(0,{kind:'init',node:0});await batch([pub(1,1),pub(0,2)]);
  }else await batch([pub(0,1)]);
  await batch([pub(0,3),pub(1,2),collect(1),read(0)]);
  await batch([pub(0,5),pub(1,4),read(1),collect(0)]);
  await batch([pub(1,3),pub(0,1),pub(1,1,'b'),read(0,'b')]);
  await batch([collect(0),collect(1),collect(0,'b'),read(1)]);
  for(const [artifact,h] of heads)for(const k of blobs.keys())if(k.startsWith(artifact+'/')&&h!==null&&Number(k.split('/')[1])<h)throw new Error('quiescent collection leaked obsolete blob');
  safety();return {name:c.name,passed:true,steps,history};
 }catch(e){return {name:c.name,passed:false,error:String(e),steps,history};}
 finally{await Promise.all(nodes.map(n=>n.process.close()));}
}
