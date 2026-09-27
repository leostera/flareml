import {defineEval,inlineFile,user} from '@leostera/evalkit';
import {piCodingAgent} from '../agents/pi-coding';
import {modeAdherence} from '../judges/grade';
import {distributedCorrectness} from '../judges/distributed';
import {languages,commonPrompt} from './specs';
import {distributedSpecification,type DistributedTask} from './distributed';
export function distributedDefinitions(task:DistributedTask){return languages.map(language=>defineEval({
 id:`${task}-${language}`,name:`${task} / ${language}`,agent:piCodingAgent,
 fixtures:[inlineFile('SPEC.md',distributedSpecification(task,language),'candidate')],
 transcript:[user(`Create a complete ${language} project from scratch. No application implementation, configuration, or tests are supplied.\n\n${commonPrompt}`)],
 scoring:[distributedCorrectness,modeAdherence],metadata:{task,language,suite:'distributed',protocolVersion:2,specRevision:2,judgeRevision:2,scenarioCount:task==='checkpoint-coordination'?84:12,scheduleSeeds:[1,17,1729,64007]},policy:{trials:1},
}));}
