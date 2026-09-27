// Copied into candidate tools/tlc with an evaluator-set AUDIT constant prepended.
declare const AUDIT:string;
import {spawn} from 'node:child_process';
import {mkdir,readFile,writeFile,appendFile,readdir,copyFile,rm} from 'node:fs/promises';
import {resolve,join,dirname,basename} from 'node:path';
import {createHash} from 'node:crypto';
const model=resolve(process.argv[2]??'design.tla'),config=resolve(process.argv[3]??model.replace(/\.tla$/,'.cfg'));
const id=`${Date.now()}-${process.pid}`;const out=resolve('.tla/runs',id);await mkdir(out,{recursive:true});
const hash=(text:string)=>createHash('sha256').update(text).digest('hex');
const source=await readFile(model,'utf8'),cfg=await readFile(config,'utf8');
await writeFile(join(out,'model.tla'),source);await writeFile(join(out,'model.cfg'),cfg);
// Retain imported local modules too, without copying app files or upstream sources.
for(const entry of await readdir(dirname(model),{withFileTypes:true}))if(entry.isFile()&&entry.name.endsWith('.tla'))await copyFile(join(dirname(model),entry.name),join(out,entry.name));
const jar=resolve(dirname(process.argv[1]!), 'tla2tools.jar');
const args=['-Xmx512m','-XX:+UseParallelGC','-cp',jar,'tlc2.TLC','-workers','1','-seed','1','-fp','0','-metadir',join(out,'states'),'-config',config,basename(model)];
let stdout='',stderr='',timedOut=false;
const child=spawn('java',args,{cwd:dirname(model),stdio:['ignore','pipe','pipe']});
child.stdout.on('data',data=>{stdout+=data;process.stdout.write(data);});child.stderr.on('data',data=>{stderr+=data;process.stderr.write(data);});
const timer=setTimeout(()=>{timedOut=true;child.kill('SIGKILL');},30000);
const code=await new Promise<number|null>((ok,fail)=>{child.once('exit',ok);child.once('error',fail);}).finally(()=>clearTimeout(timer));
const verified=!timedOut&&code===0&&stdout.includes('Model checking completed. No error has been found.');
const violation=/Invariant (\w+) is violated/.exec(stdout)?.[1]??null;
const receipt={id,model,config,modelSha256:hash(source),configSha256:hash(cfg),code,timedOut,verified,violation,status:timedOut?'INCONCLUSIVE':verified?'VERIFIED_IN_SCOPE':violation?'INVARIANT_VIOLATION':'NOT_VERIFIED',hasDeclaredInvariant:/^\s*INVARIANTS?\b/m.test(cfg),hasDeclaredTemporalProperty:/^\s*PROPERT(?:Y|IES)\b/m.test(cfg),toolRelease:'1.7.4'};
await writeFile(join(out,'stdout.log'),stdout);await writeFile(join(out,'stderr.log'),stderr);await writeFile(join(out,'receipt.json'),JSON.stringify(receipt,null,2));
await mkdir(AUDIT,{recursive:true});await appendFile(join(AUDIT,'tlc.jsonl'),JSON.stringify(receipt)+'\n');
await rm(join(out,'states'),{recursive:true,force:true});
process.exitCode=timedOut?124:code??1;
