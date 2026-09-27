import {readFile,readdir} from 'node:fs/promises';
import {join} from 'node:path';
const root='_evalkit-evidence';
for(const id of await readdir(root)){
 const folder=join(root,id);let stats:any;try{stats=JSON.parse(await readFile(join(folder,'agent.json'),'utf8'));}catch{continue;}
 if(process.argv.includes('--active')&&(stats.exitCode!==null||stats.interrupted))continue;
 const raw=await readFile(join(folder,'pi.jsonl'),'utf8');let turns=0,tokens=0,tools=0,lastTool='',lastStop='';let settled=false;
 for(const line of raw.split('\n')){if(!line)continue;let e:any;try{e=JSON.parse(line);}catch{continue;}
  if(e.type==='message_end'&&e.message?.role==='assistant'){turns++;tokens+=e.message.usage?.totalTokens??0;lastStop=e.message.stopReason;}
  if(e.type==='tool_execution_start'){tools++;lastTool=e.toolName==='bash'?String(e.args?.command??'').slice(0,100).replaceAll('\n',' '):`${e.toolName} ${e.args?.path??''}`;}
  if(e.type==='agent_settled')settled=true;
 }
 console.log(JSON.stringify({trial:id,task:stats.task,language:stats.language,mode:stats.mode,seconds:stats.durationMs?Math.round(stats.durationMs/1000):Math.round((Date.now()-Date.parse(stats.startedAt))/1000),turns,tokens,tools,lastTool,lastStop,settled}));
}
