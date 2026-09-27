import {readFile,writeFile,readdir,mkdir} from 'node:fs/promises';
import {join,resolve,relative} from 'node:path';
import {createHash} from 'node:crypto';
import {command} from '../lib/process';
const campaign=process.argv[2];if(!campaign)throw new Error('Supply campaign id');
const manifest=JSON.parse(await readFile(resolve('_evalkit-campaigns',campaign,'manifest.json'),'utf8'));
const binary=resolve('../target/release/fml');const rows=[];
for(const run of manifest.results){
 if(run.mode!=='flareml'||!run.trialId)continue;
 const root=resolve('_evalkit-results',run.runId,'trials',run.trialId,'artifacts/candidate');
 const sources:string[]=[];
 async function walk(dir:string){for(const entry of await readdir(dir,{withFileTypes:true})){if(entry.isSymbolicLink()||['.fml','target','node_modules','tools','flareml-docs','.git'].includes(entry.name))continue;const path=join(dir,entry.name);if(entry.isDirectory())await walk(path);else if(entry.name.endsWith('.fml'))sources.push(path);}}
 try{await walk(root);}catch{}
 const models=[];
 for(const [i,source] of sources.entries()){
  const content=await readFile(source,'utf8');
  const artifacts=resolve('_evalkit-evidence',run.trialId,'model-audit',String(i));
  const result=await command([binary,'check',source,'--format','json','--max-states','10000','--timeout','5s','--artifacts-dir',artifacts],root,process.env,15000);
  let report:any;try{report=JSON.parse(result.stdout);}catch{}
  models.push({path:relative(root,source),sha256:createHash('sha256').update(content).digest('hex'),exitCode:result.code,status:report?.status??'UNPARSEABLE_OUTPUT',complete:report?.complete??false,states:report?.states,edges:report?.edges,claims:report?.claims?.map((c:any)=>({name:c.name,kind:c.kind,result:c.result})),hasNonCoverClaim:report?.claims?.some((c:any)=>c.kind==='Invariant'||c.kind==='Property')??false,hasReachedCover:report?.claims?.some((c:any)=>c.kind==='Cover'&&c.result==='REACHED')??false,stderr:result.stderr.slice(-2000)});
 }
 const savedReports:Record<string,number>={};
 try{for(const entry of await readdir(join(root,'.fml/runs'),{withFileTypes:true})){if(!entry.isDirectory())continue;try{const saved=JSON.parse(await readFile(join(root,'.fml/runs',entry.name,'report.json'),'utf8'));savedReports[saved.status??'unknown']=(savedReports[saved.status??'unknown']??0)+1;}catch{}}}catch{}
 rows.push({eval:run.eval,trialId:run.trialId,savedReports,models});
}
const out=resolve('reports',campaign);await mkdir(out,{recursive:true});await writeFile(join(out,'model-audit.json'),JSON.stringify({note:'Supplemental rerun of candidate-authored models outside saved artifacts/docs. Does not establish model adequacy or code correspondence. Original grades are unchanged.',limits:{maxStates:10000,timeout:'5s'},rows},null,2)+'\n');console.log(JSON.stringify(rows.map(r=>({eval:r.eval,models:r.models.map(m=>({path:m.path,status:m.status,states:m.states,hasNonCoverClaim:m.hasNonCoverClaim,hasReachedCover:m.hasReachedCover}))})),null,2));
