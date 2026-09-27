// Reproduce an upstream distributed-design regression, not a coding-agent trial.
import {mkdir,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
import {command} from '../lib/process';
const revision='c9e45d0e4695a8552d31105b93dff21456b7be69';
const base=`https://raw.githubusercontent.com/tlaplus/Examples/${revision}`;
const relative='specifications/CheckpointCoordination';
const jarUrl='https://github.com/tlaplus/tlaplus/releases/download/v1.7.4/tla2tools.jar';
const jarSha='936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88';
const sha=(data:Uint8Array)=>createHash('sha256').update(data).digest('hex');
async function download(url:string){const r=await fetch(url);if(!r.ok)throw new Error(`${r.status}: ${url}`);return new Uint8Array(await r.arrayBuffer());}
const directory=resolve('_evalkit-evidence','source-research',`checkpoint-${Date.now()}`);await mkdir(directory,{recursive:true});
const files=['CheckpointCoordination.tla','MCCheckpointCoordination.tla','MCCheckpointCoordination.cfg','MCCheckpointCoordinationFailure.cfg','README.md'];
const data=await Promise.all(files.map(name=>download(`${base}/${relative}/${name}`)));
const license=await download(`${base}/LICENSE.md`);await writeFile(join(directory,'LICENSE.md'),license);
const jar=await download(jarUrl);if(sha(jar)!==jarSha)throw new Error('Unexpected TLC binary checksum');const jarPath=join(directory,'tla2tools.jar');await writeFile(jarPath,jar);
const rows=[];
for(const variant of ['buggy','fixed'] as const){
 const cwd=join(directory,variant);await mkdir(cwd);for(const [i,name] of files.entries())await writeFile(join(cwd,name),data[i]!);
 const cfg=variant==='buggy'?'MCCheckpointCoordinationFailure.cfg':'MCCheckpointCoordination.cfg';
 const start=Date.now();const result=await command(['java','-Xmx768m','-XX:+UseParallelGC','-cp',jarPath,'tlc2.TLC','-workers','1','-seed','1','-fp','0','-config',cfg,'MCCheckpointCoordination'],cwd,process.env,120000);
 await writeFile(join(cwd,'stdout.log'),result.stdout);await writeFile(join(cwd,'stderr.log'),result.stderr);
 const verified=result.code===0&&result.stdout.includes('Model checking completed. No error has been found.');
 const violation=/Invariant (\w+) is violated/.exec(result.stdout)?.[1]??null;
 const counts=/(\d[\d,]*) states generated, (\d[\d,]*) distinct states found/.exec(result.stdout);
 rows.push({variant,exitCode:result.code,verified,violation,durationMs:Date.now()-start,generated:counts?Number(counts[1]!.replaceAll(',','')):null,distinct:counts?Number(counts[2]!.replaceAll(',','')):null,evidence:cwd});
}
const report={kind:'upstream-source-diagnostic-not-agent-evaluation',source:`https://github.com/tlaplus/Examples/tree/${revision}/${relative}`,revision,license:'MIT; original license retained with diagnostic sources',sourceHashes:Object.fromEntries(files.map((name,i)=>[name,sha(data[i]!)])),tlc:{release:'1.7.4',sha256:jarSha},scope:{nodes:3,majority:2,maxLog:3,maxNat:5,constraint:'OpenIndices /= {}',symmetry:'NodeSymmetry',heapMiB:768,workers:1,timeoutSeconds:120},limitations:['Uses upstream bounded configurations, including state constraints and symmetry.','Safety reproduction, not a liveness proof or a new discovery.','No coding agent participated; neither original source nor known solution is a candidate fixture.'],rows};
const out=resolve('reports','source-research');await mkdir(out,{recursive:true});await writeFile(join(out,'checkpoint.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report,null,2));
if(rows[0]?.violation!=='SafetyInvariant'||!rows[1]?.verified)process.exitCode=1;
