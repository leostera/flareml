import {test,expect} from 'bun:test';
import {resolve} from 'node:path';
import {cases} from '../judges/cases';
import {scenario,validStorageValue} from '../judges/scheduler';
import {initial,transition,linearizations} from '../judges/reference';
import {tasks} from '../tasks/specs';
import {fixtures} from '../lib/fixtures';

test('candidate fixtures contain a specification only, no starter project',()=>{
 for(const task of tasks)for(const language of ['rust','go','typescript'] as const){const f=fixtures(task,language);expect(f).toHaveLength(1);expect(JSON.stringify(f)).toContain('SPEC.md');}
});
test('storage limit is UTF-8 bytes, not JavaScript code units',()=>{
 expect(validStorageValue(null)).toBe(false);
 expect(validStorageValue(undefined)).toBe(false);
 expect(validStorageValue('x'.repeat(65534))).toBe(true);
 expect(validStorageValue('x'.repeat(65535))).toBe(false);
 expect(validStorageValue('🦀'.repeat(20000))).toBe(false);
});
test('oracle known answers include prototype keys and exact refund semantics',()=>{
 let state=initial('payments',{balance:10});
 let next=transition('payments',state,{op:'charge',key:'__proto__',amount:4});expect(next.result).toEqual({status:'charged'});expect(next.state.balance).toBe(6);
 next=transition('payments',next.state,{op:'refund',key:'__proto__'});expect(next.state.balance).toBe(10);
 const retry=transition('payments',next.state,{op:'charge',key:'__proto__',amount:4});expect(retry.result).toEqual({status:'refunded'});expect(retry.state.balance).toBe(10);
 expect(transition('payments',next.state,{op:'charge',key:'__proto__',amount:5}).result).toEqual({status:'conflict'});
 const url=transition('url-shortener',initial('url-shortener',{}),{op:'shorten',key:'constructor',url:'u',candidates:['__proto__']});expect(url.result).toEqual({status:'ok',code:'__proto__'});expect(Object.hasOwn(url.state.urls,'__proto__')).toBe(true);
});
test('linearizability oracle rejects overdraft and respects completed-before-invoked ordering',()=>{
 const state=initial('inventory',{stock:1});
 const operations=[{request:{op:'reserve',id:'a',quantity:1},result:{status:'reserved'},start:0,end:1},{request:{op:'reserve',id:'b',quantity:1},result:{status:'reserved'},start:2,end:3}];
 expect(linearizations('inventory',[state],operations)).toHaveLength(0);
 const staleRead={request:{op:'inspect'},result:{status:'ok',available:1,reservations:{}},start:2,end:3};
 expect(linearizations('inventory',[state],[operations[0]!,staleRead])).toHaveLength(0);
 // The same old snapshot is legal if the read overlaps the reservation.
 expect(linearizations('inventory',[state],[operations[0]!,{...staleRead,start:0}])).toHaveLength(1);
});
test('trusted CAS worker passes every frozen case and mutation controls fail',async()=>{
 const worker=resolve('tests/reference-worker.ts');
 for(const task of tasks){
  for(const c of cases(task)){const result=await scenario(task,c,['bun',worker,task,'correct'],process.cwd());expect(result.error,result.name).toBeUndefined();expect(result.passed).toBe(true);}
  for(const mutant of ['no-idempotency','ignore-cas-failure','reply-before-commit']){
   let failures=0;for(const c of cases(task).filter(c=>c.seed===17)){const result=await scenario(task,c,['bun',worker,task,mutant],process.cwd());if(!result.passed)failures++;}
   expect(failures,`${task}/${mutant} must be caught`).toBeGreaterThan(0);
  }
 }
 for(const [task,mutant] of [['inventory','oversell'],['payments','refund-twice']] as const){let failures=0;for(const c of cases(task).filter(c=>c.seed===17))if(!(await scenario(task,c,['bun',worker,task,mutant],process.cwd())).passed)failures++;expect(failures).toBeGreaterThan(0);}
},120000);
