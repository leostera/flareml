import {readFile,writeFile,mkdir,readdir,copyFile} from 'node:fs/promises';
import {join,resolve,relative} from 'node:path';
import {createHash} from 'node:crypto';
const id=process.argv[2];if(!id)throw new Error('Supply campaign id');
const out=resolve('_evalkit-campaigns',id,'harness');await mkdir(out,{recursive:false});
const hashes:Record<string,string>={};
async function capture(path:string){for(const entry of await readdir(path,{withFileTypes:true})){const file=join(path,entry.name);if(entry.isDirectory())await capture(file);else if(entry.isFile()&&(/\.(ts|json|md)$/.test(entry.name))){const rel=relative(process.cwd(),file);const data=await readFile(file);hashes[rel]=createHash('sha256').update(data).digest('hex');await mkdir(join(out,rel,'..'),{recursive:true});await writeFile(join(out,rel),data);}}}
for(const dir of ['agents','judges','lib','tasks','evals','tests','scripts'])await capture(resolve(dir));
for(const file of ['evalkit.config.ts','package.json','bun.lock','tsconfig.json','README.md']){const data=await readFile(file);hashes[file]=createHash('sha256').update(data).digest('hex');await copyFile(file,join(out,file));}
await writeFile(join(out,'hashes.json'),JSON.stringify(hashes,null,2)+'\n');console.log(`${Object.keys(hashes).length} harness files snapshotted`);
