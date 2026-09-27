import {defineAgent,type JsonValue} from '@leostera/evalkit';
import {spawn,type ChildProcess} from 'node:child_process';
import {mkdir,copyFile,chmod,writeFile,rm} from 'node:fs/promises';
import {createWriteStream} from 'node:fs';
import {join,resolve} from 'node:path';
import {materializeManual} from '../lib/fixtures';
import {materializeTla} from '../lib/tla';
const timestamp=()=>new Date().toISOString();
const json=(value:unknown):JsonValue=>JSON.parse(JSON.stringify(value));
export const piCodingAgent=defineAgent({
 identity:{id:'pi-gpt-six-luna',kind:'local-process',name:'Local Pi / GPT-6-LUNA'},
 runtimes:{local:{kind:'local-process'}},
 async start({context,onEvent}){
  const parameters=context.parameters??{};
  if(parameters.model!=='openai-codex/gpt-6-luna')throw new Error('Exact model must be openai-codex/gpt-6-luna');
  if(parameters.mode!=='baseline'&&parameters.mode!=='flareml'&&parameters.mode!=='tla')throw new Error('Invalid treatment mode');
  const mode=parameters.mode;
  const audit=resolve('_evalkit-evidence',context.trialId);await mkdir(audit,{recursive:true});
  const root=context.workspace.root;
  const stats={model:parameters.model,mode,task:context.metadata.task,language:context.metadata.language,startedAt:timestamp(),durationMs:0,input:0,output:0,cacheRead:0,cacheWrite:0,totalTokens:0,reportedCost:0,costIsInvoice:false,assistantTurns:0,toolCalls:0,flaremlCalls:0,tlaCalls:0,checkCalls:0,modelReports:[] as string[],stopReason:'not-started',exitCode:null as number|null,budgetStop:null as string|null,compactions:0,usageComplete:true};
  await writeFile(join(audit,'agent.json'),JSON.stringify(stats,null,2));
  if(mode==='flareml'){
   await mkdir(join(root,'tools'),{recursive:true});
   await copyFile(resolve('../target/release/fml'),join(root,'tools/fml'));await chmod(join(root,'tools/fml'),0o755);
   await materializeManual(root);
  }
  if(mode==='tla')await materializeTla(root,audit);
  let child:ChildProcess|undefined;
  const stop=()=>{if(child?.pid){try{process.kill(-child.pid,'SIGTERM');}catch{}}};
  await onEvent({kind:'started',timestamp:timestamp()});
  return {
   async send(message:string){
    const modePrompt=mode==='flareml'?`\n\nCondition: FlareML-assisted. BEFORE implementing application logic, model the concurrency design using ./tools/fml, check it, and refine it as needed. Read flareml-docs/SKILL.md and relevant manuals. flareml-docs/demo.fml shows the current syntax. Save your task-specific model(s) as design.fml (or similarly named files), retain .fml/runs evidence, and explain abstraction bounds and the model-to-code mapping in DESIGN.md. Include meaningful safety properties and a reachable progress scenario; do not treat an inconclusive result as verified. Use bounded checks (e.g. --max-states 10000 --timeout 5s) while iterating. FlareML does not prove your eventual implementation correct. After the model check, implement and test the requested language program. Use only FlareML as your formal checker, not TLA+/TLC.`:mode==='tla'?`\n\nCondition: TLA+-assisted. BEFORE implementing application logic, author and check a task-specific TLA+ (or PlusCal translated to TLA+) model. Read tla-docs/README.md. Use ./tools/tlc MODEL.tla MODEL.cfg for all TLC checks so invocations and evidence are retained. Save models and configurations, retain .tla/runs, and explain bounds, assumptions and the model-to-code mapping in DESIGN.md. Check meaningful safety properties and an explicitly labeled reachable progress scenario. A counterexample to a negated reachability target is a cover witness, not a discovered bug. A timeout or parser error is not verification. Model separate storage/persistence/message steps where the implementation has them. After checking/refining, implement and test the program. Use only TLA+/TLC as your formal checker, not FlareML.`:`\n\nCondition: baseline. Implement and test directly using normal programming tools. Do not use FlareML, formal model checkers, or read their documentation. You may reason about the design and write ordinary tests.`;
    const budget=`\n\nBudget: ${Number(parameters.chatTimeoutMs)/60000} minutes; at most ${parameters.turnBudget} assistant turns and ${parameters.maxTokens} provider-reported total tokens. Keep modeling bounded and leave time for implementation and testing. No subagents or additional model calls. Finish with a concise summary.\n`;
    const prompt=message+modePrompt+budget;await writeFile(join(audit,'prompt.txt'),prompt);
    const args=['--mode','json','--no-session','--no-extensions','--no-skills','--no-prompt-templates','--no-themes','--no-context-files','--no-approve','--offline','--provider','openai-codex','--model','gpt-6-luna','--thinking','high','--tools','read,bash,edit,write','--system-prompt','You are an expert coding agent implementing the task in the supplied workspace. Use tools to read, edit, build, and test files. Stay within the candidate workspace and normal toolchain caches. Do not inspect evaluator files or other workspaces, invoke other AI agents, or change machine/user configuration. Never claim tests passed without running them.',prompt];
    const raw=createWriteStream(join(audit,'pi.jsonl')),stderr=createWriteStream(join(audit,'stderr.log'));
    const start=Date.now();let buffer='',lastStop='',settled=false,streamError:unknown;
    child=spawn(process.env.PI_EVAL_COMMAND??'pi',args,{cwd:root,env:{...process.env,PI_OFFLINE:'1',PATH:mode==='flareml'?join(root,'tools')+':'+process.env.PATH:process.env.PATH},stdio:['ignore','pipe','pipe'],detached:true});
    const exit=new Promise<number|null>((resolve,reject)=>{child!.once('exit',resolve);child!.once('error',reject);});
    // Attach a rejection observer before consuming streams.
    void exit.catch(()=>{});
    child.stderr!.on('data',chunk=>stderr.write(chunk));
    let killTimer:ReturnType<typeof setTimeout>|undefined;
    const abort=(reason:string)=>{if(stats.budgetStop)return;stats.budgetStop=reason;stats.usageComplete=false;stop();killTimer=setTimeout(()=>{try{process.kill(-child!.pid!,'SIGKILL');}catch{}},2000);};
    const timer=setTimeout(()=>abort('wall-time'),Number(parameters.chatTimeoutMs));
    const usage=(u:any)=>{if(!u)return;stats.input+=u.input??0;stats.output+=u.output??0;stats.cacheRead+=u.cacheRead??0;stats.cacheWrite+=u.cacheWrite??0;stats.totalTokens+=u.totalTokens??0;stats.reportedCost+=u.cost?.total??0;};
    await onEvent({kind:'turn-started',turn:1,timestamp:timestamp()});
    try{
     child.stdout!.setEncoding('utf8');
     for await(const chunk of child.stdout!){raw.write(chunk);buffer+=chunk;let end;
      while((end=buffer.indexOf('\n'))>=0){const line=buffer.slice(0,end).replace(/\r$/,'');buffer=buffer.slice(end+1);if(!line)continue;const event=JSON.parse(line);
       if(event.type==='message_end'&&event.message?.role==='assistant'){
        const m=event.message;usage(m.usage);stats.assistantTurns++;lastStop=m.stopReason;
        if(!stats.modelReports.includes(`${m.provider}/${m.model}`))stats.modelReports.push(`${m.provider}/${m.model}`);
        const text=(m.content??[]).filter((c:any)=>c.type==='text').map((c:any)=>c.text).join('\n');
        if(text)await onEvent({kind:'message',role:'assistant',content:text,timestamp:timestamp()});
        if(m.model!=='gpt-6-luna'||m.provider!=='openai-codex')abort('unexpected-model');
       }else if(event.type==='tool_execution_start'){
        stats.toolCalls++;if(event.toolName==='bash'&&/\b(?:tlc|tlc2|tla2tools)\b/i.test(event.args?.command??''))stats.tlaCalls++;if(event.toolName==='bash'&&/(?:\bfml\b|flareml)/i.test(event.args?.command??'')){stats.flaremlCalls++;if(/\bcheck\b/.test(event.args.command))stats.checkCalls++;}
        await onEvent({kind:'tool-call',id:event.toolCallId,name:event.toolName,arguments:json(event.args),timestamp:timestamp()});
       }else if(event.type==='tool_execution_end')await onEvent({kind:'tool-result',id:event.toolCallId,name:event.toolName,result:json(event.result),timestamp:timestamp()});
       else if(event.type==='compaction_end'&&event.result){stats.compactions++;usage(event.result.usage);}
       else if(event.type==='entry_appended'&&event.entry?.type==='usage')usage(event.entry.usage);
       else if(event.type==='agent_settled')settled=true;
       if(stats.totalTokens>Number(parameters.maxTokens))abort('tokens');
       if(stats.assistantTurns>Number(parameters.turnBudget))abort('assistant-turns');
      }
     }
    }catch(e){streamError=e;abort('stream-error');}
    finally{stats.exitCode=await exit.catch(e=>{streamError=e;return -1;});clearTimeout(timer);clearTimeout(killTimer);raw.end();stderr.end();stats.durationMs=Date.now()-start;stats.stopReason=lastStop;
     await writeFile(join(audit,'agent.json'),JSON.stringify(stats,null,2));
     await onEvent({kind:'turn-completed',turn:1,latencyMs:stats.durationMs,usage:{inputTokens:stats.input+stats.cacheRead+stats.cacheWrite,outputTokens:stats.output,totalTokens:stats.totalTokens},timestamp:timestamp()});
    }
    if(streamError||stats.budgetStop||stats.exitCode!==0||!settled||lastStop==='error'||lastStop==='aborted'||!stats.assistantTurns)throw new Error(`Pi run failed: ${String(streamError??stats.budgetStop??lastStop)}, exit=${stats.exitCode}, settled=${settled}`);
   },
   async close(){stop();for(const dir of ['target','node_modules','tools'])await rm(join(root,dir),{recursive:true,force:true});await onEvent({kind:'completed',output:json(stats),timestamp:timestamp()});},
  };
 }
});
