// Independent short wire transcripts; builds unchanged saved sources, never repairs them.
import {mkdir,writeFile,readFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {build} from '../judges/grade';
import {Lines} from '../lib/process';
const campaign=process.argv[2]??'distributed-001';
const summary=JSON.parse(await readFile(resolve('reports',campaign,'summary.json'),'utf8'));
const out=resolve('_evalkit-evidence',`${campaign}-wire-reproductions`);await mkdir(out,{recursive:true});const results=[];
for(const [language,mode,node,now] of [['go','baseline',1,0],['go','flareml',2,0],['rust','tla',1,4]] as const){
 const row=summary.rows.find((r:any)=>r.eval===`checkpoint-coordination-${language}`&&r.mode===mode);if(!row)continue;
 const root=resolve(row.report,'trials',row.trialId,'artifacts/candidate');
 const b=await build(language,root,join(out,`${language}-${mode}`));if(b.code!==0)throw new Error(b.stderr);
 const process=new Lines(b.command,root);const transcript=[];
 try{
  const init={kind:'init',node,nodes:[0,1,2],leader:mode==='baseline'?0:node,epoch:1,now:0,completed:[]};transcript.push({input:init,output:await process.send(init)});
  const tick={kind:'tick',now};const output:any=await process.send(tick);transcript.push({input:tick,output});
  if(language==='rust'){const result={kind:'result',now,token:output[0].token,ok:false,entry:null};transcript.push({input:result,output:await process.send(result)});}
 }finally{await process.close();}
 results.push({eval:row.eval,mode,originalTrialId:row.trialId,transcript});
}
await writeFile(resolve('reports',campaign,'wire-reproductions.json'),JSON.stringify({note:'Actual responses from unchanged saved candidates. These are diagnostics, not replacement trials.',results},null,2)+'\n');console.log(JSON.stringify(results,null,2));
