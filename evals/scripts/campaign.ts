import {Effect} from 'effect';
import {runEval,localReportStore} from '@leostera/evalkit/runner';
import {mkdir,writeFile,appendFile,readFile} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {definitions} from '../tasks/definition';
import {tasks} from '../tasks/specs';
import config from '../evalkit.config';
import {distributedTasks} from '../tasks/distributed';
import {distributedDefinitions} from '../tasks/distributed-definition';
import {ensureTla,tlaSha,tlaRelease} from '../lib/tla';
const arg=(name:string)=>process.argv.find(a=>a.startsWith(`--${name}=`))?.split('=').slice(1).join('=');
const id=arg('id')??new Date().toISOString().replaceAll(':','-').replaceAll('.','-');
if(!/^[a-zA-Z0-9_.-]+$/.test(id))throw new Error('Invalid campaign id');
await mkdir(resolve('_evalkit-campaigns'),{recursive:true});
const directory=resolve('_evalkit-campaigns',id);await mkdir(directory,{recursive:false});
execFileSync('bun',['scripts/snapshot-harness.ts',id],{stdio:'inherit'});
const versions=Object.fromEntries([['pi',['pi','--version']],['rust',['rustc','--version']],['go',['go','version']],['bun',['bun','--version']],['python',['python3','--version']],['java',['java','--version']],['flareml',['../target/release/fml','--version']]].map(([name,args])=>[name,execFileSync((args as string[])[0]!, (args as string[]).slice(1),{encoding:'utf8'}).trim()]));
const suite=arg('suite')??'simple';
if(!['simple','distributed','all'].includes(suite))throw new Error('Unknown suite');
const registry=[...(suite!=='distributed'?tasks.flatMap(definitions):[]),...(suite!=='simple'?distributedTasks.flatMap(distributedDefinitions):[])];
const selected=registry.filter(d=>(!arg('only')||d.id===arg('only'))&&(!arg('languages')||arg('languages')!.split(',').includes(String(d.metadata?.language))));
if(!selected.length)throw new Error('No matching evals');
const modes=(arg('modes')??'baseline,flareml,tla').split(',');
if(new Set(modes).size!==modes.length||modes.some(m=>!['baseline','flareml','tla'].includes(m)))throw new Error('Invalid modes');
if(modes.includes('tla'))await ensureTla();
// Rotate order across task/language groups; never run agents concurrently.
const cells=selected.flatMap((definition,i)=>[...modes.slice(i%modes.length),...modes.slice(0,i%modes.length)].map(mode=>({definition,mode})));
let stopRequested=false;
for(const signal of ['SIGINT','SIGTERM'] as const)process.on(signal,()=>{stopRequested=true;console.log('Stop requested: finishing the current trial, then stopping before the next cell.');});
const manifest={id,suite,tlaTool:modes.includes('tla')?{release:tlaRelease,sha256:tlaSha}:null,startedAt:new Date().toISOString(),status:'running',versions,sourceCommit:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),binarySha256:createHash('sha256').update(await readFile('../target/release/fml')).digest('hex'),model:'openai-codex/gpt-6-luna',thinking:'high',trialsPerCell:1,limits:config.matrix!.defaults,scheduleSeeds:[1,17,1729,64007],cells:cells.map(c=>({eval:c.definition.id,mode:c.mode,metadata:c.definition.metadata})),results:[] as unknown[]};
await writeFile(join(directory,'manifest.json'),JSON.stringify(manifest,null,2));
console.log(`Campaign ${id}: ${cells.length} sequential cells`);
for(const [index,{definition,mode}] of cells.entries()){
 if(stopRequested||await Bun.file(join(directory,'STOP')).exists()){stopRequested=true;break;}
 const parameters={...config.matrix!.defaults,model:'openai-codex/gpt-6-luna',mode};
 console.log(`[${index+1}/${cells.length}] START ${definition.id} ${mode} ${new Date().toISOString()}`);
 try{
  const result=await Effect.runPromise(runEval(definition,{report:localReportStore(resolve('_evalkit-results')),workspaceRoot:join(tmpdir(),'flareml-evalkit-workspaces'),runtime:'local',trials:1,concurrency:1,parameters,matrix:{id:'flareml-impact',cellKey:JSON.stringify([definition.id,parameters])}}));
  const record={eval:definition.id,mode,...result};manifest.results.push(record);await appendFile(join(directory,'runs.jsonl'),JSON.stringify(record)+'\n');
  console.log(`[${index+1}/${cells.length}] DONE ${definition.id} ${mode}: execution=${result.status} scoring=${result.scoring?.passed} report=${result.reportLocation}`);
 }catch(error){const record={eval:definition.id,mode,infrastructureError:String(error)};manifest.results.push(record);await appendFile(join(directory,'runs.jsonl'),JSON.stringify(record)+'\n');console.error(record);}
 await writeFile(join(directory,'manifest.json'),JSON.stringify(manifest,null,2));
}
manifest.status=stopRequested?'stopped':'completed';await writeFile(join(directory,'manifest.json'),JSON.stringify(manifest,null,2));
console.log(`${manifest.status.toUpperCase()} ${directory}`);
