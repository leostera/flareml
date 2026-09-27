import {inlineFile} from '@leostera/evalkit';
import {readdir,mkdir,copyFile,writeFile} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {specification,type Task,type Language} from '../tasks/specs';
// No Cargo.toml, go.mod, package.json, entrypoint, implementation, or tests.
// Pi must create the whole project. Only the public contract is a fixture.
export function fixtures(task:Task,language:Language){return [inlineFile('SPEC.md',specification(task,language),'candidate')];}
export async function materializeManual(root:string){
 const source=resolve('../docs/skills/fml');
 await mkdir(join(root,'flareml-docs'),{recursive:true});
 for(const name of await readdir(source))if(name.endsWith('.md'))await copyFile(join(source,name),join(root,'flareml-docs',name));
 // Tool documentation example, not a candidate project or task solution.
 await writeFile(join(root,'flareml-docs','demo.fml'),`actor Bit { init(): Bool { false } handle_message(state: Bool, message: unit): Bool { true } }\nproperty "eventually set" { reachable (exists (b in instances(Bit)) { b.state == Some(true) }) }\ncheck Demo { spawn_bound Bit = 1 mailbox_bound = 1 main { let b = spawn(Bit); send(b, ()); } }\n`);
}
