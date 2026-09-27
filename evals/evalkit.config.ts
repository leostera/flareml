import {defineConfig} from '@leostera/evalkit';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
export default defineConfig({
 matrix:{id:'flareml-impact',parameters:{mode:['baseline','flareml','tla'],model:['openai-codex/gpt-6-luna']},defaults:{maxTokens:600000,turnBudget:80,chatTimeoutMs:720000}},
 execution:{concurrency:1,trials:1,maxCells:60},
 sandboxDir:join(tmpdir(),'flareml-evalkit-workspaces'),
});
