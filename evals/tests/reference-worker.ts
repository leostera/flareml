// Trusted harness self-test only. Never copied to candidate workspaces.
import {createInterface} from 'node:readline';
import {initial,transition} from '../judges/reference';
import type {Task} from '../tasks/specs';
const task=process.argv[2] as Task,mutant=process.argv[3]??'correct';
let config:any,serial=0;const jobs=new Map<string,any>();
function read(job:any){const token=String(serial++);jobs.set(token,{...job,phase:'get'});return [{kind:'get',key:'document',token}];}
function prepare(job:any,value:any){
 const before=value??initial(task,config);const modified=structuredClone(before);
 if(mutant==='no-idempotency'){
  if(task==='url-shortener'&&job.request.op==='shorten')delete modified.requests[job.request.key];
  if(task==='inventory'&&job.request.op==='reserve')delete modified.reservations[job.request.id];
  if(task==='payments'&&job.request.op==='charge')delete modified.payments[job.request.key];
 }
 if(mutant==='refund-twice'&&task==='payments'&&job.request.op==='refund'&&Object.hasOwn(modified.payments,job.request.key))modified.payments[job.request.key].status='charged';
 if(mutant==='oversell'&&task==='inventory'&&job.request.op==='reserve'&&modified.available<job.request.quantity)modified.available=job.request.quantity;
 const {state,result}=transition(task,modified,job.request);
 const token=String(serial++);jobs.set(token,{...job,phase:'cas',result});
 const cas={kind:'cas',key:'document',expected:value,value:state,token};
 return mutant==='reply-before-commit'?[{kind:'reply',id:job.id,result},cas]:[cas];
}
for await(const line of createInterface({input:process.stdin})){
 const e=JSON.parse(line);let actions:any[]=[];
 if(e.kind==='init')config=e.config;
 else if(e.kind==='request')actions=read(e);
 else{const job=jobs.get(e.token);jobs.delete(e.token);
  if(job.phase==='get')actions=prepare(job,e.value);
  else if(mutant==='reply-before-commit')actions=[];
  else if(e.ok||mutant==='ignore-cas-failure')actions=[{kind:'reply',id:job.id,result:job.result}];
  else actions=prepare(job,e.value);
 }
 console.log(JSON.stringify(actions));
}
