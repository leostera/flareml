// Evaluator-only wire adapter for a semantics-preserving node-ID permutation.
import {createInterface} from 'node:readline';
import {Lines} from '../lib/process';
const permutation=process.argv[2]!.split(',').map(Number);
if(permutation.length!==3||new Set(permutation).size!==3||permutation.some(x=>![0,1,2].includes(x)))throw new Error('Expected permutation of 0,1,2');
const inverse=permutation.map((_,i)=>permutation.indexOf(i));
function rename(value:any,map:number[]):any{
 if(Array.isArray(value))return value.map(v=>rename(v,map));
 if(value===null||typeof value!=='object')return value;
 return Object.fromEntries(Object.entries(value).map(([key,v])=>[key,['node','leader','holder'].includes(key)&&typeof v==='number'&&map[v]!==undefined?map[v]:rename(v,map)]));
}
const child=new Lines(process.argv.slice(3),process.cwd());
try{for await(const line of createInterface({input:process.stdin})){const response=await child.send(rename(JSON.parse(line),permutation));console.log(JSON.stringify(rename(response,inverse)));}}finally{await child.close();}
