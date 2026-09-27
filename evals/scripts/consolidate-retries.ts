// Reporting only: joins explicitly planned infrastructure retries. No agent/tool execution.
import {readFile,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
const id=process.argv[2];if(!id)throw new Error('Supply campaign ID');
const root=resolve('reports',id);
const original=JSON.parse(await readFile(join(root,'summary.json'),'utf8'));
const plan=JSON.parse(await readFile(join(root,'retry-plan.json'),'utf8'));
const retryCampaigns=[...new Set<string>(plan.cells.map((c:any)=>c.retryCampaign))];
const summaries=new Map<string,any>();
for(const campaign of retryCampaigns){const s=JSON.parse(await readFile(resolve('reports',campaign,'summary.json'),'utf8'));if(s.status!=='completed')throw new Error(`Incomplete retry ${campaign}`);summaries.set(campaign,s);}
const replaced=new Set<string>();
const rows=original.rows.map((r:any)=>{
 const cell=plan.cells.find((c:any)=>c.originalTrialId===r.trialId);
 if(!cell)return {...r,sourceCampaign:id};
 const retry=summaries.get(cell.retryCampaign).rows.filter((x:any)=>x.eval===r.eval&&x.mode===r.mode);
 if(retry.length!==1)throw new Error('Expected exactly one planned retry');
 replaced.add(r.trialId);return {...retry[0],sourceCampaign:cell.retryCampaign,replacesInfrastructureTrial:r.trialId};
});
if(replaced.size!==plan.cells.length||rows.length!==original.expectedCells)throw new Error('Incomplete comparison');
const attempts=[...original.rows.map((r:any)=>({...r,sourceCampaign:id})),...[...summaries].flatMap(([campaign,s])=>s.rows.map((r:any)=>({...r,sourceCampaign:campaign})))];
const sum=(xs:any[],key:string)=>xs.reduce((n,r)=>n+(r[key]??0),0);
const aggregate=Object.fromEntries(['baseline','flareml','tla'].map(mode=>{
 const xs=rows.filter((r:any)=>r.mode===mode),all=attempts.filter((r:any)=>r.mode===mode);
 return [mode,{cells:xs.length,passingImplementations:xs.filter((r:any)=>r.correctnessPassed).length,normalCompletions:xs.filter((r:any)=>r.execution==='completed').length,budgetStops:xs.filter((r:any)=>r.budgetStop).length,agentSeconds:sum(xs,'agentSeconds'),tokens:sum(xs,'tokens'),input:sum(xs,'input'),output:sum(xs,'output'),cacheRead:sum(xs,'cacheRead'),reportedCostUSD:sum(xs,'reportedCostUSD'),allAttempts:all.length,allAttemptAgentSeconds:sum(all,'agentSeconds'),allAttemptTokens:sum(all,'tokens'),allAttemptReportedCostUSD:sum(all,'reportedCostUSD')}];
}));
const comparison={campaign:id,note:'24 cells, with four provider-transport failures replaced only by their explicitly planned fresh full-agent retries. All 28 attempts remain listed. Coding failures and budget stops were not retried. Costs are reported estimates, not invoices; failed/interrupted requests may have unreported usage.',retryPlan:plan,aggregate,rows,attempts};
await writeFile(join(root,'comparison.json'),JSON.stringify(comparison,null,2)+'\n');
console.log(JSON.stringify(aggregate,null,2));
