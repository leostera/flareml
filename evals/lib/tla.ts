import {mkdir,copyFile,writeFile,readFile,chmod} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
export const tlaRelease='1.7.4';
export const tlaSha='936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88';
export const tlaJar=()=>resolve('_evalkit-tools/tla2tools-1.7.4.jar');
export async function ensureTla(){
 await mkdir(resolve('_evalkit-tools'),{recursive:true});let data:Uint8Array;
 try{data=await readFile(tlaJar());}catch{const response=await fetch(`https://github.com/tlaplus/tlaplus/releases/download/v${tlaRelease}/tla2tools.jar`);if(!response.ok)throw new Error(`TLC download: ${response.status}`);data=new Uint8Array(await response.arrayBuffer());}
 if(createHash('sha256').update(data).digest('hex')!==tlaSha)throw new Error('TLC checksum mismatch');await writeFile(tlaJar(),data);
}
export async function materializeTla(root:string,audit:string){
 await mkdir(join(root,'tools'),{recursive:true});await copyFile(tlaJar(),join(root,'tools/tla2tools.jar'));
 // Evaluator-authored runner records actual calls separately from shell keyword counts.
 const runner=await readFile(resolve('lib/tlc-runner.ts'),'utf8');
 await writeFile(join(root,'tools/tlc'),`#!/usr/bin/env bun\nconst AUDIT=${JSON.stringify(audit)};\n`+runner);await chmod(join(root,'tools/tlc'),0o755);
 await mkdir(join(root,'tla-docs'),{recursive:true});
 await writeFile(join(root,'tla-docs/README.md'),`# TLA+ / TLC quickstart (tool documentation, not a task solution)
Write a .tla module and .cfg file yourself. Use ./tools/tlc design.tla design.cfg to check; the module declaration must match the filename. The wrapper limits each check to 30 seconds, one worker, 512MiB, and records stdout, stderr, source/configuration snapshots, and a receipt in .tla/runs/. All checking counts toward your total agent budget. It forwards no arbitrary CLI arguments; configure the finite model in the cfg. For PlusCal, translate with java -cp tools/tla2tools.jar pcal.trans YourModule.tla before checking the generated TLA+.

TLA+ uses /\\ for conjunction, \\/ for disjunction, primed variables for next state, UNCHANGED <<x,y>> for unchanged components, and EXCEPT for function updates. Every action must specify the next value of every variable. Define Init, Next, vars == <<...>>, and Spec == Init /\\ [][Next]_vars, adding explicitly justified fairness if checking progress. Constants and finite domains belong in the cfg. INVARIANT names state predicates; PROPERTY names temporal formulas. Do not prove only TypeOK or omit real storage/message steps. Separate reads, conditional writes, persistence acknowledgements, and message delivery where those are separate in the implementation.

Generic syntax example (not a service design):
---- MODULE Bit ----
VARIABLE bit
vars == <<bit>>
Init == bit = FALSE
Next == bit' = TRUE
Spec == Init /\\ [][Next]_vars /\\ WF_vars(Next)
TypeOK == bit \\in BOOLEAN
EventuallySet == <>bit
====

Bit.cfg:
SPECIFICATION Spec
INVARIANT TypeOK
PROPERTY EventuallySet

For an existential reachability witness, check a SEPARATE configuration containing a negated target invariant (e.g. NotTarget == ~Target). Its violation is an intentional witness, NOT a design bug. Preserve and label it separately. Include real safety checks in your normal cfg. Deadlock checking is on by default; model legitimate termination with an enabled stuttering action or explicitly explain disabling it via CHECK_DEADLOCK FALSE. STATE constraints prune behavior: declare and justify them, never use them to exclude a bad behavior. A timeout/parse error is not verification. The wrapper's verified status only reports successful TLC completion; humans still need to review properties, assumptions, and model/code correspondence.
`);
}
