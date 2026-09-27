import {defineEval,user} from '@leostera/evalkit';
import {piCodingAgent} from '../agents/pi-coding';
import {fixtures} from '../lib/fixtures';
import {correctness,modeAdherence} from '../judges/grade';
import {commonPrompt,languages,type Task} from './specs';
export function definitions(task:Task){return languages.map(language=>defineEval({
 id:`${task}-${language}`,name:`${task} / ${language}`,
 agent:piCodingAgent,fixtures:fixtures(task,language),transcript:[user(`Start a new ${language==='rust'?'Rust':language==='go'?'Go':language==='python'?'Python 3':'TypeScript on Bun'} project from scratch in this workspace. No application code, project configuration, or test scaffolding is supplied.\n\n${commonPrompt}`)],
 scoring:[correctness,modeAdherence],metadata:{task,language,protocolVersion:1,judgeRevision:2,scheduleSeeds:[1,17,1729,64007]},policy:{trials:1},
}));}
