import {test,expect} from 'bun:test';
import {mkdtemp,writeFile,readFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {materializeTla,tlaJar} from '../lib/tla';
import {command,Lines} from '../lib/process';
import {build} from '../judges/grade';
// Deliberately no downloads/provider calls in tests. Run setup first for TLC smoke.
test('Python builds and runs through the same process protocol',async()=>{
 const root=await mkdtemp(join(tmpdir(),'eval-python-'));
 try{await writeFile(join(root,'main.py'),'import sys\nfor line in sys.stdin:\n print("[]", flush=True)\n');const b=await build('python',root,join(root,'build'));expect(b.code).toBe(0);const p=new Lines(b.command,root);try{expect(await p.send({kind:'init'})).toEqual([]);}finally{await p.close();}}finally{await rm(root,{recursive:true,force:true});}
});
const hasTla=await Bun.file(tlaJar()).exists();
test.skipIf(!hasTla)('TLC runner records actual verification and a real counterexample',async()=>{
 const root=await mkdtemp(join(tmpdir(),'eval-tlc-'));const audit=join(root,'audit');
 try{
  await materializeTla(root,audit);
  await writeFile(join(root,'Bit.tla'),'---- MODULE Bit ----\nVARIABLE bit\nvars == <<bit>>\nInit == bit = FALSE\nNext == bit\' = TRUE\nSpec == Init /\\ [][Next]_vars /\\ WF_vars(Next)\nTypeOK == bit \\in BOOLEAN\nWrong == bit = FALSE\nEventuallySet == <>bit\n====\n');
  await writeFile(join(root,'good.cfg'),'SPECIFICATION Spec\nINVARIANT TypeOK\nPROPERTY EventuallySet\n');
  await writeFile(join(root,'bad.cfg'),'SPECIFICATION Spec\nINVARIANT Wrong\n');
  const good=await command(['./tools/tlc','Bit.tla','good.cfg'],root);expect(good.code,good.stderr).toBe(0);
  const bad=await command(['./tools/tlc','Bit.tla','bad.cfg'],root);expect(bad.code).not.toBe(0);
  const records=(await readFile(join(audit,'tlc.jsonl'),'utf8')).trim().split('\n').map(line=>JSON.parse(line));expect(records[0].verified).toBe(true);expect(records[1].violation).toBe('Wrong');expect(records[1].verified).toBe(false);
 }finally{await rm(root,{recursive:true,force:true});}
},45000);
