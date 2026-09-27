// Research diagnostic, not an agent trial or a replacement for EvalKit.
// Candidate workspaces must never receive these upstream solution models.
import {mkdir,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
import {command} from '../lib/process';
const revision='ca30663506a7e18de9d23fabbc70a369903d7c98';
const base=`https://raw.githubusercontent.com/elastic/elasticsearch-formal-models/${revision}`;
const jarUrl='https://github.com/tlaplus/tlaplus/releases/download/v1.7.4/tla2tools.jar';
const jarSha='936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88';
const sha=(data:Uint8Array|string)=>createHash('sha256').update(data).digest('hex');
async function download(url:string){const response=await fetch(url);if(!response.ok)throw new Error(`${response.status}: ${url}`);return new Uint8Array(await response.arrayBuffer());}
const directory=resolve('_evalkit-evidence','source-research',`elastic-storage-${Date.now()}`);
await mkdir(directory,{recursive:true});
const [sourceBytes,license,jar]=await Promise.all([download(`${base}/Storage/tla/Storage.tla`),download(`${base}/LICENSE`),download(jarUrl)]);
if(sha(jar)!==jarSha)throw new Error('Unexpected TLC binary checksum');
const source=new TextDecoder().decode(sourceBytes);
await writeFile(join(directory,'LICENSE'),license);
const jarPath=join(directory,'tla2tools.jar');await writeFile(jarPath,jar);
const fragment='/\\ DeleteNewManifestEasy)';
if(source.split(fragment).length!==2)throw new Error('Upstream mutation location must be unique');
const rows=[];
for(const variant of ['fixed','buggy'] as const){
 const cwd=join(directory,variant);await mkdir(cwd);
 const model=variant==='fixed'?source:source.replace(fragment,'/\\ DeleteNewManifestBuggy)');
 await writeFile(join(cwd,'Storage.tla'),model);
 await writeFile(join(cwd,'StorageCheck.tla'),'---- MODULE StorageCheck ----\nEXTENDS Storage\nBound == newMeta < MaxNewMeta\n====\n');
 await writeFile(join(cwd,'StorageCheck.cfg'),'CONSTANTS\n MaxNewMeta = 3\n MetaDataContent = MetaDataContent\nINIT Init\nNEXT Next\nCONSTRAINT Bound\nINVARIANTS\n MetadataFileReferencedByManifestExists\n MetadataReferencedByManifestIsValid\nCHECK_DEADLOCK FALSE\n');
 const args=['java','-Xmx512m','-XX:+UseParallelGC','-cp',jarPath,'tlc2.TLC','-workers','1','-seed','1','-fp','0','StorageCheck'];
 const start=Date.now();const result=await command(args,cwd,process.env,60000);
 await writeFile(join(cwd,'stdout.log'),result.stdout);await writeFile(join(cwd,'stderr.log'),result.stderr);
 const verified=result.code===0&&result.stdout.includes('Model checking completed. No error has been found.');
 const violation=/Invariant (\w+) is violated/.exec(result.stdout)?.[1]??null;
 const counts=/(\d[\d,]*) states generated, (\d[\d,]*) distinct states found/.exec(result.stdout);
 rows.push({variant,exitCode:result.code,verified,violation,durationMs:Date.now()-start,modelSha256:sha(model),generated:counts?Number(counts[1]!.replaceAll(',','')):null,distinct:counts?Number(counts[2]!.replaceAll(',','')):null,evidence:cwd});
}
const report={kind:'upstream-source-diagnostic-not-agent-evaluation',source:`${base}/Storage/tla/Storage.tla`,revision,upstreamSourceSha256:sha(sourceBytes),license:'Apache-2.0; original license retained with diagnostic sources',tlc:{version:'1.7.4',url:jarUrl,sha256:jarSha},scope:{maxNewMeta:3,stateConstraint:'newMeta < MaxNewMeta',deadlockChecking:false,workers:1,heapMiB:512,timeoutSeconds:60},limitations:['Bounded, state-constrained safety check; not an unbounded proof or liveness check.','This upstream model has a single control flow with uncertain storage outcomes, not a multi-node distributed implementation.','Switches between upstream-provided cleanup variants; no agent designed or repaired these models.'],rows};
const out=resolve('reports','source-research');await mkdir(out,{recursive:true});await writeFile(join(out,'elastic-storage.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report,null,2));
if(!rows[0]?.verified||rows[1]?.violation!=='MetadataFileReferencedByManifestExists')throw new Error('Expected fixed control to verify and documented bad cleanup to violate referential integrity');
