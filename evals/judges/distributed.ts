import {predicate,type ScoreValue} from '@leostera/evalkit';
import {mkdir,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {build} from './grade';
import {checkpointCases,checkpointScenario,type DistributedResult} from './checkpoint';
import {manifestCases,manifestScenario} from './manifest';
import type {Language} from '../tasks/specs';
export const distributedCorrectness=predicate('independent distributed safety and progress',async({context,artifacts}):Promise<ScoreValue>=>{
 const audit=resolve('_evalkit-evidence',context.trialId);await mkdir(audit,{recursive:true});
 const root=artifacts.candidate.root;const built=await build(context.metadata.language as Language,root,join(artifacts.evaluator.root,'build'));
 await writeFile(join(audit,'build.json'),JSON.stringify(built,null,2));
 if(built.code!==0)return {value:0,passed:false,explanation:'Candidate build failed',evidence:{buildExit:built.code,stderr:built.stderr.slice(-6000)}};
 const results:DistributedResult[]=[];
 if(context.metadata.task==='checkpoint-coordination'){
  for(const permutation of ['0,1,2','1,2,0','2,0,1'])for(const c of checkpointCases){
   const argv=permutation==='0,1,2'?built.command:['bun',resolve('judges/checkpoint-rename.ts'),permutation,...built.command];
   const result=await checkpointScenario(c,argv,root);results.push({...result,name:`${permutation}/${result.name}`});
  }
 }
 else if(context.metadata.task==='manifest-publication')for(const c of manifestCases)results.push(await manifestScenario(c,built.command,root));
 else throw new Error('Unknown distributed task');
 await writeFile(join(audit,'cases.json'),JSON.stringify(results,null,2));const passed=results.filter(r=>r.passed).length;
 return {value:passed/results.length,passed:passed===results.length,explanation:`${passed}/${results.length} distributed safety/progress schedules passed`,evidence:{passed,total:results.length,failures:results.filter(r=>!r.passed).map(r=>({name:r.name,error:r.error??'unknown',steps:r.steps}))}};
},{supportsPartial:true});
