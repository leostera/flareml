// Private positive/mutation control. Never materialized as a candidate fixture.
import {createInterface} from 'node:readline';
const task=process.argv[2],mutant=process.argv[3]??'correct';let node=0,leader=0,epoch=0,now=0,latest:any=null,active:number|null=null,pending=false;
let notBefore=0;let completed=new Set<number>(),attempted=new Set<number>();let serial=0;const callbacks=new Map<string,(e:any)=>void>();let actions:any[]=[];
function storage(kind:string,args:any,done:(e:any)=>void){const token=`t${serial++}`;callbacks.set(token,done);actions.push({kind,...args,token});}
function reply(id:string,result:any){actions.push({kind:'reply',id,result});}
function publish(id:string,r:any){
 const head=()=>storage('head-read',{artifact:r.artifact},e=>{
  const h=e.value;if(h!==null&&h>r.version&&mutant!=='regress-head')return reply(id,{status:'superseded'});
  if(h===r.version)return reply(id,{status:'published'});
  storage('head-cas',{artifact:r.artifact,expected:h,version:r.version},e=>{
   if(e.status==='unknown'&&mutant==='cleanup-uncertain')return storage('blob-delete',{artifact:r.artifact,version:r.version},()=>reply(id,{status:'published'}));
   head();
  });
 });
 const put=()=>storage('blob-put',{artifact:r.artifact,version:r.version,payload:r.payload},e=>e.status==='ok'?head():storage('blob-read',{artifact:r.artifact,version:r.version},e=>e.value===r.payload?head():put()));put();
}
function read(id:string,r:any){storage('head-read',{artifact:r.artifact},e=>{if(e.value===null)return reply(id,{status:'missing'});const version=e.value;storage('blob-read',{artifact:r.artifact,version},e=>{if(e.value===null){if(mutant==='stale-missing')reply(id,{status:'missing'});else read(id,r);}else reply(id,{status:'ok',version,payload:e.value});});});}
function collect(id:string,r:any){if(mutant==='leak')return reply(id,{status:'ok'});storage('head-read',{artifact:r.artifact},e=>{const h=e.value;storage('blob-list',{artifact:r.artifact},e=>{const versions=e.value.filter((v:number)=>h!==null&&(mutant==='delete-current'?v<=h:v<h));const remove=()=>{const version=versions.shift();if(version===undefined)return reply(id,{status:'ok'});const erase=()=>storage('blob-delete',{artifact:r.artifact,version},e=>e.status==='ok'?remove():storage('blob-read',{artifact:r.artifact,version},e=>e.value===null?remove():erase()));erase();};remove();});});}
function coordinate(){
 if(node===leader){
  if(active!==null){if(mutant!=='skip-promotion-abort')actions.push({kind:'abort',seq:active});active=null;}
  if(mutant==='never-work'||now<notBefore)return;
  if(!pending&&(!latest||latest.until<=now||(mutant==='preempt'&&latest.holder===node))){let holder=latest?(latest.holder+1)%3:(node+1)%3;while(holder===leader)holder=(holder+1)%3;if(mutant==='no-recovery-fence-reset')holder=(node+1+serial%2)%3;if(mutant==='fixed-holders')holder=latest?.holder===1?2:1;pending=true;actions.push({kind:'propose',token:`t${serial++}`,epoch,holder,until:now+(mutant==='short-leases'?2:4)});}
 }else if(latest&&latest.holder===node&&latest.until>now&&!completed.has(latest.seq)&&!attempted.has(latest.seq)&&active===null){active=latest.seq;attempted.add(latest.seq);actions.push({kind:'start',seq:latest.seq});}
}
for await(const line of createInterface({input:process.stdin})){
 const e=JSON.parse(line);actions=[];
 if(task==='manifest-publication'){
  if(e.kind==='init'){callbacks.clear();node=e.node;}
  else if(e.kind==='request'){const r=e.request;if(r.op==='publish')publish(e.id,r);else if(r.op==='read')read(e.id,r);else collect(e.id,r);}
  else if(e.kind==='result'){const cb=callbacks.get(e.token);callbacks.delete(e.token);cb?.(e);}
 }else{
  now=e.now;
  if(e.kind==='init'){node=e.node;leader=e.leader;epoch=e.epoch;completed=new Set(mutant==='forget-completions'?[]:e.completed);notBefore=mutant.startsWith('no-recovery-fence')?now:now+4;latest=null;active=null;pending=false;attempted=new Set();}
  else{
   if(e.kind==='view'&&e.epoch>=epoch){leader=e.leader;epoch=e.epoch;pending=false;}
   if(e.kind==='apply'||e.kind==='result'&&e.ok){const entry=e.entry;if(!latest||entry.seq>latest.seq)latest=entry;}
   if(e.kind==='result')pending=false;
   if(e.kind==='finished'){if(e.outcome==='completed')completed.add(e.seq);if(active===e.seq)active=null;}
   coordinate();
  }
 }
 console.log(JSON.stringify(actions));
}
