import {test,expect} from 'bun:test';
import {resolve} from 'node:path';
import {checkpointCases,checkpointScenario} from '../judges/checkpoint';
import {manifestCases,manifestScenario} from '../judges/manifest';
const worker=resolve('tests/distributed-worker.ts');

test('checkpoint: reference passes all 84 schedules, including all stable primary IDs',async()=>{
 expect(checkpointCases.length).toBe(28);
 for(const permutation of ['0,1,2','1,2,0','2,0,1']){
  const argv=permutation==='0,1,2'?['bun',worker,'checkpoint-coordination']:['bun',resolve('judges/checkpoint-rename.ts'),permutation,'bun',worker,'checkpoint-coordination'];
  for(const c of checkpointCases){const r=await checkpointScenario(c,argv,process.cwd());expect(r.error,`${permutation}/${r.name}`).toBeUndefined();}
 }
},120000);

test('checkpoint: two-unit leases complete at deadline; specific safety mutants are rejected',async()=>{
 for(const c of checkpointCases){const r=await checkpointScenario(c,['bun',worker,'checkpoint-coordination','short-leases'],process.cwd());expect(r.error,r.name).toBeUndefined();}
 for(const [variant,mutant,error] of [
  ['recovery-backlog','no-recovery-fence','overlapping checkpoints'],
  ['recovery-backlog-2','no-recovery-fence-reset','overlapping checkpoints'],
  ['completed-replay','forget-completions','invalid checkpoint start'],
  ['promotion-active','skip-promotion-abort','primary checkpointing'],
 ])for(const c of checkpointCases.filter(c=>c.variant===variant)){
  const r=await checkpointScenario(c,['bun',worker,'checkpoint-coordination',mutant!],process.cwd());expect(r.passed).toBe(false);expect(r.error).toContain(error!);
 }
 for(const mutant of ['preempt','never-work']){
  const r=await checkpointScenario(checkpointCases[0]!,['bun',worker,'checkpoint-coordination',mutant],process.cwd());expect(r.passed,mutant).toBe(false);
 }
 for(const permutation of ['1,2,0','2,0,1']){
  const r=await checkpointScenario(checkpointCases[0]!,['bun',resolve('judges/checkpoint-rename.ts'),permutation,'bun',worker,'checkpoint-coordination','fixed-holders'],process.cwd());expect(r.passed).toBe(false);
 }
},120000);

test('manifest: all 12 corrected scenarios pass the reference and reject unsafe controls',async()=>{
 expect(manifestCases.length).toBe(12);
 expect(new Set(manifestCases.map(c=>c.variant))).toEqual(new Set(['uncertain-publication','inflight-restart','focused-reader-gap']));
 for(const c of manifestCases){const r=await manifestScenario(c,['bun',worker,'manifest-publication'],process.cwd());expect(r.error,r.name).toBeUndefined();}
 for(const c of manifestCases.filter(c=>c.variant==='focused-reader-gap')){
  const r=await manifestScenario(c,['bun',worker,'manifest-publication','stale-missing'],process.cwd());expect(r.passed).toBe(false);expect(r.error).toContain('invalid missing read');
 }
 for(const mutant of ['cleanup-uncertain','regress-head','delete-current','leak']){
  let failed=0;for(const c of manifestCases.filter(c=>c.seed===17)){const r=await manifestScenario(c,['bun',worker,'manifest-publication',mutant],process.cwd());if(!r.passed)failed++;}expect(failed,mutant).toBeGreaterThan(0);
 }
},120000);
