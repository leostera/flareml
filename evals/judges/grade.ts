import {mkdir,writeFile,readFile,readdir} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {predicate,type ScoreValue} from '@leostera/evalkit';
import {command} from '../lib/process';
import {cases} from './cases';
import {scenario} from './scheduler';
import type {Language,Task} from '../tasks/specs';
export async function build(language:Language,root:string,out:string){
 await mkdir(out,{recursive:true});
 if(language==='rust'){
  const result=await command(['cargo','build','--offline'],root,{...process.env,CARGO_TARGET_DIR:join(out,'target')});
  return {...result,command:[join(out,'target/debug/service')]};
 }
 if(language==='go'){
  const path=join(out,'service');const result=await command(['go','build','-o',path,'.'],root,{...process.env,GOTOOLCHAIN:'local'});
  return {...result,command:[path]};
 }
 if(language==='python'){
  const result=await command(['python3','-c','from pathlib import Path; compile(Path("main.py").read_text(), "main.py", "exec")'],root);
  return {...result,command:['python3','-u',join(root,'main.py')]};
 }
 // Bun runs TS directly; a build/transpile smoke check catches syntax errors.
 const result=await command(['bun','build','./main.ts','--target=bun','--outfile',join(out,'service.js')],root);
 return {...result,command:['bun',join(out,'service.js')]};
}
export const correctness=predicate('independent linearizability and durability',async({context,artifacts}):Promise<ScoreValue>=>{
 const task=context.metadata.task as Task,language=context.metadata.language as Language;
 const root=artifacts.candidate.root,out=join(artifacts.evaluator.root,'build');
 const audit=resolve('_evalkit-evidence',context.trialId);await mkdir(audit,{recursive:true});
 const built=await build(language,root,out);await writeFile(join(audit,'build.json'),JSON.stringify(built,null,2));
 if(built.code!==0)return {value:0,passed:false,explanation:'Candidate build failed',evidence:{buildExit:built.code,stderr:built.stderr.slice(-6000)}};
 const results=[];
 for(const test of cases(task))results.push(await scenario(task,test,built.command,root));
 await writeFile(join(audit,'cases.json'),JSON.stringify(results,null,2));
 const passed=results.filter(r=>r.passed).length;
 return {value:passed/results.length,passed:passed===results.length,explanation:`${passed}/${results.length} fixed adversarial schedules passed`,evidence:{passed,total:results.length,failures:results.filter(r=>!r.passed).map(r=>({name:r.name,error:r.error??'unknown',steps:r.steps}))}};
},{supportsPartial:true});
export const modeAdherence=predicate('assigned workflow observed',async({context}):Promise<ScoreValue>=>{
 const audit=resolve('_evalkit-evidence',context.trialId);
 const stats=JSON.parse(await readFile(join(audit,'agent.json'),'utf8'));
 if(context.parameters?.mode==='baseline')return {value:stats.flaremlCalls===0&&(stats.tlaCalls??0)===0?1:0,explanation:`${stats.flaremlCalls} FlareML / ${stats.tlaCalls??0} TLC keyword commands in baseline`,evidence:{flaremlCalls:stats.flaremlCalls,tlaCalls:stats.tlaCalls??0}};
 if(context.parameters?.mode==='tla'){
  let receipts:any[]=[];try{receipts=(await readFile(join(audit,'tlc.jsonl'),'utf8')).trim().split('\n').filter(Boolean).map(line=>JSON.parse(line));}catch{}
  const verified=receipts.filter(r=>r.verified&&r.hasDeclaredInvariant&&!r.model.includes('/tla-docs/')).length;
  return {value:verified>0&&stats.flaremlCalls===0?1:0,explanation:`${receipts.length} recorded TLC invocations; ${verified} completed checks with declared invariants (not proof adequacy)`,evidence:{invocations:receipts.length,verified}};
 }
 const root=context.workspace.root;
 let verified=0,models=0;
 async function scan(path:string,depth=0){if(depth>8)return;for(const entry of await readdir(path,{withFileTypes:true})){if(entry.isSymbolicLink()||['target','node_modules','.git','tools','flareml-docs'].includes(entry.name))continue;const p=join(path,entry.name);if(entry.isDirectory())await scan(p,depth+1);else if(entry.name.endsWith('.fml'))models++;else if(entry.name==='report.json'){try{const report=JSON.parse(await readFile(p,'utf8'));if(report.status==='VERIFIED_IN_SCOPE'&&report.complete===true)verified++;}catch{}}}}
 await scan(root);
 return {value:stats.flaremlCalls>0&&verified>0&&models>0?1:0,explanation:`${stats.flaremlCalls} observed calls; ${models} saved models; ${verified} complete verified reports (not implementation proofs)`,evidence:{calls:stats.flaremlCalls,models,verified}};
},{supportsPartial:true});
