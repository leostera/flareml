import {readFile,readdir,writeFile,mkdir} from 'node:fs/promises';
import {resolve,join} from 'node:path';
const campaign=process.argv[2];if(!campaign)throw new Error('Supply campaign id');
const manifest=JSON.parse(await readFile(resolve('_evalkit-campaigns',campaign,'manifest.json'),'utf8'));
const rows=[];
for(const run of manifest.results){
 if(run.mode==='baseline'||!run.trialId)continue;
 const root=resolve('_evalkit-results',run.runId,'trials',run.trialId,'artifacts/candidate');
 const checks:any[]=[];
 if(run.mode==='flareml'){
  try{for(const dir of await readdir(join(root,'.fml/runs'),{withFileTypes:true})){if(!dir.isDirectory())continue;try{const report=JSON.parse(await readFile(join(root,'.fml/runs',dir.name,'report.json'),'utf8'));checks.push({id:dir.name,status:report.status,complete:report.complete??false,states:report.states,error:report.error?.message??null,claims:report.claims?.map((c:any)=>({name:c.name,kind:c.kind,result:c.result})),source:join('.fml/runs',dir.name,'model.fml')});}catch{}}}catch{}
 }else{
  try{const records=(await readFile(resolve('_evalkit-evidence',run.trialId,'tlc.jsonl'),'utf8')).trim().split('\n').filter(Boolean).map(s=>JSON.parse(s));for(const r of records){let cfg='';try{cfg=await readFile(join(root,'.tla/runs',r.id,'model.cfg'),'utf8');}catch{}checks.push({...r,configurationText:cfg,source:join('.tla/runs',r.id,'model.tla'),classificationNote:r.violation?'Requires review: may be an intentional negated reachability witness, not a design bug.':null});}}catch{}
 }
 const statuses:Record<string,number>={};for(const c of checks)statuses[c.status]=(statuses[c.status]??0)+1;
 rows.push({eval:run.eval,mode:run.mode,trialId:run.trialId,statuses,checks});
}
const out=resolve('reports',campaign);await mkdir(out,{recursive:true});await writeFile(join(out,'formal-history.json'),JSON.stringify({note:'Saved history only. Counts are not a bug-discovery score. Review counterexamples, property changes, assumptions and subsequent code/test changes.',rows},null,2)+'\n');console.log(JSON.stringify(rows.map(r=>({eval:r.eval,mode:r.mode,statuses:r.statuses})),null,2));
