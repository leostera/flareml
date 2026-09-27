import {Lines} from '../lib/process';
export type DistributedResult={name:string;passed:boolean;error?:string;steps:number;history:any[]};
type Lease={seq:number;epoch:number;holder:number;until:number};
export const checkpointCases=[1,17,1729,64007].flatMap(seed=>['handover-replay','lag-and-stale-results','restart-primary','recovery-backlog-2','recovery-backlog','completed-replay','promotion-active'].map(name=>({name:`${name}/seed-${seed}`,seed,variant:name})));
export async function checkpointScenario(c:typeof checkpointCases[number],command:string[],cwd:string):Promise<DistributedResult>{
 let now=0,epoch=1,leader=0,steps=0,random=c.seed>>>0;const history:any[]=[];
 const next=()=>random=(Math.imul(random,1664525)+1013904223)>>>0;
 const log:Lease[]=[];
 const nodes=Array.from({length:3},()=>({process:new Lines(command,cwd),inc:0,alive:true,cursor:0,learned:new Set<number>(),completed:new Set<number>(),active:null as {seq:number;until:number;finish:number}|null,tokens:new Set<string>()}));
 const queue:any[]=[];
 function safety(){const active=nodes.filter(n=>n.alive&&n.active);if(active.length>1)throw new Error('overlapping checkpoints');if(nodes[leader]!.active)throw new Error('primary checkpointing');}
 function record(value:any){if(++steps>5000)throw new Error('5000-event bound');history.push({step:steps,now,epoch,leader,...value});}
 async function input(i:number,event:any){const n=nodes[i]!;if(!n.alive)return;record({node:i,input:event});const raw=await n.process.send({...event,now});if(!Array.isArray(raw)||raw.length>64)throw new Error('expected bounded action array');
  if(event.kind==='init'&&raw.length)throw new Error('init must return []');
  for(const a of raw){record({node:i,action:a});if(!a||typeof a!=='object')throw new Error('invalid action');
   const fields=a.kind==='propose'?['kind','token','epoch','holder','until']:['kind','seq'];if(Object.keys(a).some(k=>!fields.includes(k)))throw new Error('extra action fields');
   if(a.kind==='propose'){
    if(typeof a.token!=='string'||a.token.length>512||n.tokens.has(a.token)||![a.epoch,a.holder,a.until].every(Number.isInteger)||a.holder<0||a.holder>2)throw new Error('invalid proposal/token');
    n.tokens.add(a.token);queue.push({kind:'append',node:i,inc:n.inc,action:a});
   }else if(a.kind==='start'){
    const entry=log[a.seq-1];if(!Number.isInteger(a.seq)||!entry||!n.learned.has(a.seq)||entry.holder!==i||entry.until<=now||n.active||n.completed.has(a.seq)||i===leader)throw new Error('invalid checkpoint start');
    n.active={seq:a.seq,until:entry.until,finish:now+2};
   }else if(a.kind==='abort'){if(n.active?.seq===a.seq)n.active=null;}
   else throw new Error('unknown checkpoint action');
   safety();
  }safety();
 }
 async function init(i:number){const n=nodes[i]!;await input(i,{kind:'init',node:i,nodes:[0,1,2],leader,epoch,completed:[...n.completed]});}
 async function apply(i:number){const n=nodes[i]!;if(!n.alive||n.active||n.cursor>=log.length)return false;const entry=log[n.cursor++]!;n.learned.add(entry.seq);await input(i,{kind:'apply',entry});return true;}
 async function delivery(item:any){const n=nodes[item.node]!;
  if(item.kind==='append'){
   const a=item.action;const ok=item.node===leader&&a.epoch===epoch&&a.holder!==leader&&a.until>now&&a.until<=now+4;
   const entry=ok?{seq:log.length+1,epoch,holder:a.holder,until:a.until}:null;if(entry)log.push(entry);
   record({append:item.action,accepted:ok,entry});queue.push({kind:'result',node:item.node,inc:item.inc,token:a.token,ok,entry});return;
  }
  if(!n.alive||n.inc!==item.inc)return;
  if(item.kind==='result'){n.tokens.delete(item.token);if(item.entry)n.learned.add(item.entry.seq);await input(item.node,{kind:'result',token:item.token,ok:item.ok,entry:item.entry});}
  else await input(item.node,item.event);
 }
 async function pump(count:number){for(let k=0;k<count;k++){
  const choices:any[]=queue.map((_,i)=>({q:i}));for(let i=0;i<3;i++){const n=nodes[i]!;if(n.alive&&!n.active&&n.cursor<log.length)choices.push({apply:i});}
  if(!choices.length)return;const choice=choices[next()%choices.length]!;if(choice.apply!==undefined)await apply(choice.apply);else await delivery(queue.splice(choice.q,1)[0]);
 }}
 async function drain(){await pump(40);} // Do not starve clock ticks behind continuous traffic.
 async function tick(){now++;for(let i=0;i<3;i++){const n=nodes[i]!;if(n.active&&(n.active.until<=now||n.active.finish<=now)){const a=n.active;n.active=null;const outcome=a.finish<=a.until&&a.finish<=now?'completed':'aborted';if(outcome==='completed')n.completed.add(a.seq);queue.push({kind:'event',node:i,inc:n.inc,event:{kind:'finished',seq:a.seq,outcome}});}}for(let i=0;i<3;i++)if(nodes[i]!.alive)await input(i,{kind:'tick'});await drain();}
 async function elect(i:number){const n=nodes[i]!;
  // Catch up before promotion, but do not pre-abort caught-up active work.
  // The candidate's promotion response must actually abort it.
  let waits=0;while(n.cursor<log.length){if(!(await apply(i))){if(++waits>32)throw new Error('replica failed to catch up under healthy delivery');await tick();}}
  leader=i;epoch++;record({election:i});await input(i,{kind:'view',leader,epoch});
  for(let j=0;j<3;j++)if(j!==i)queue.push({kind:'event',node:j,inc:nodes[j]!.inc,event:{kind:'view',leader,epoch}});
 }
 async function restart(i:number){const n=nodes[i]!;record({restart:i});await n.process.close();n.inc++;n.active=null;n.cursor=0;n.learned.clear();n.tokens.clear();n.process=new Lines(command,cwd);await init(i);}
 try{
  for(let i=0;i<3;i++)await init(i);await input(0,{kind:'view',leader,epoch});await drain();
  for(let k=0;k<32&&!log.length;k++)await tick();
  if(!log.length)throw new Error('primary never issued work');
  if(c.variant.startsWith('recovery-backlog')){
   const required=c.variant==='recovery-backlog-2'?2:3;
   for(let k=0;k<32&&(log.length<required||!nodes.some(n=>n.active));k++)await tick();
   if(log.length<required)throw new Error('primary did not create the required lease prefix under healthy delivery');
   await restart(leader);
   // Replay an expired prefix, allowing a resulting append before the tail.
   await apply(leader);await input(leader,{kind:'tick'});
   const pending=queue.findIndex(x=>x.kind==='append'&&x.node===leader&&x.inc===nodes[leader]!.inc);
   if(pending>=0)await delivery(queue.splice(pending,1)[0]);
   for(let i=0;i<3;i++)if(i!==leader)while(await apply(i)){}
   await drain();
  }else if(c.variant==='promotion-active'){
   for(let k=0;k<32&&!nodes.some(n=>n.active&&n.cursor===log.length);k++)await tick();
   const holder=nodes.findIndex(n=>n.active&&n.cursor===log.length);
   if(holder<0)throw new Error('no caught-up active checkpoint under healthy delivery');
   await elect(holder);await drain();
  }else if(c.variant==='completed-replay'){
   let holder=-1;
   for(let k=0;k<32;k++){
    holder=nodes.findIndex(n=>[...n.completed].some(seq=>log[seq-1]!.until>now));
    if(holder>=0)break;await tick();
   }
   // Two-unit leases may have no unexpired completed lease to replay.
   if(holder>=0){await restart(holder);await drain();}
  }else if(c.variant==='handover-replay'){
   const holder=log[0]!.holder;await elect(holder);await drain();
   await elect(0);await restart(holder);await drain();
  }else if(c.variant==='lag-and-stale-results'){
   await tick();await input(leader,{kind:'tick'});await pump(1);await elect((leader+1)%3);await pump(2);await restart((leader+1)%3);await drain();
   // Deliver a genuinely stale view after the new epoch.
   await input(leader,{kind:'view',leader:0,epoch:1});
  }else{await restart(leader);await drain();await elect((leader+1)%3);await drain();}
  for(let k=0;k<5;k++)await tick();await elect(0);await drain();
  const before=nodes.map(n=>n.completed.size);for(let k=0;k<32;k++)await tick();
  for(const i of [1,2])if(nodes[i]!.completed.size-before[i]!<2)throw new Error(`healthy secondary ${i} did not complete two checkpoints`);
  return {name:c.name,passed:true,steps,history};
 }catch(e){return {name:c.name,passed:false,error:String(e),steps,history};}
 finally{await Promise.all(nodes.map(n=>n.process.close()));}
}
