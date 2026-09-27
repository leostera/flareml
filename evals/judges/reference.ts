import type {Task} from '../tasks/specs';
export type Json = null|boolean|number|string|Json[]|{[key:string]:Json};
export type Request = {op:string;[key:string]:Json};
export type State = Record<string,any>;
const own=(o:State,k:string)=>Object.hasOwn(o,k)?o[k]:undefined;
const put=(o:State,k:string,v:unknown)=>Object.defineProperty(o,k,{value:v,enumerable:true,configurable:true,writable:true});
export function canonical(value:unknown):string{if(value===null||typeof value!=='object')return JSON.stringify(value);if(Array.isArray(value))return '['+value.map(canonical).join(',')+']';return '{'+Object.entries(value).sort(([a],[b])=>a<b?-1:a>b?1:0).map(([k,v])=>JSON.stringify(k)+':'+canonical(v)).join(',')+'}';}
export function initial(task:Task,config:State):State{return task==='url-shortener'?{urls:{},requests:{}}:task==='inventory'?{available:config.stock,reservations:{}}:{balance:config.balance,payments:{}};}
export function transition(task:Task,before:State,r:Request):{state:State;result:Json}{
 const state=structuredClone(before);let status='';
 if(r.op==='inspect')return {state,result:{status:'ok',...state}};
 if(task==='url-shortener'){
  if(r.op==='resolve'){const url=own(state.urls,r.code as string);return {state,result:url===undefined?{status:'missing'}:{status:'ok',url}};}
  const key=r.key as string,url=r.url as string,existing=own(state.requests,key);
  if(existing)return {state,result:existing.url===url?{status:'ok',code:existing.code}:{status:'conflict'}};
  const code=(r.candidates as string[]).find(c=>own(state.urls,c)===undefined||own(state.urls,c)===url);
  if(code===undefined)return {state,result:{status:'exhausted'}};
  put(state.urls,code,url);put(state.requests,key,{url,code});return {state,result:{status:'ok',code}};
 }
 if(task==='inventory'){
  const id=r.id as string,entry=own(state.reservations,id);
  if(r.op==='reserve'){
   if(entry)status=entry.quantity===r.quantity?entry.status:'conflict';
   else if((r.quantity as number)>state.available)status='insufficient';
   else{state.available-=r.quantity as number;put(state.reservations,id,{quantity:r.quantity,status:'reserved'});status='reserved';}
  }else if(!entry)status='missing';
  else if(entry.status!=='reserved')status=entry.status;
  else if(r.op==='confirm'){entry.status='confirmed';status='confirmed';}
  else{state.available+=entry.quantity;entry.status='cancelled';status='cancelled';}
 }else{
  const key=r.key as string,entry=own(state.payments,key);
  if(r.op==='charge'){
   if(entry)status=entry.amount===r.amount?entry.status:'conflict';
   else if((r.amount as number)>state.balance)status='insufficient';
   else{state.balance-=r.amount as number;put(state.payments,key,{amount:r.amount,status:'charged'});status='charged';}
  }else if(!entry)status='missing';
  else if(entry.status==='refunded')status='refunded';
  else{state.balance+=entry.amount;entry.status='refunded';status='refunded';}
 }
 return {state,result:{status}};
}
export type Observation={request:Request;result:Json;start:number;end:number};
// Exhaustive serial reference, constrained by real-time precedence. Independent
// of candidate KV layout and implementation algorithm. Max batch size is four.
export function linearizations(task:Task,states:State[],operations:Observation[]):State[]{
 const ends=new Map<string,State>();
 function visit(state:State,remaining:number[]){
  if(!remaining.length){ends.set(canonical(state),state);return;}
  for(const i of remaining){const op=operations[i]!;
   if(remaining.some(j=>j!==i&&operations[j]!.end<op.start))continue;
   const next=transition(task,state,op.request);
   if(canonical(next.result)===canonical(op.result))visit(next.state,remaining.filter(j=>j!==i));
  }
 }
 for(const state of states)visit(state,operations.map((_,i)=>i));
 if(ends.size>512)throw new Error('Judge linearization state limit exceeded');
 return [...ends.values()];
}
