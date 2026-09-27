import {spawn,type ChildProcessWithoutNullStreams} from 'node:child_process';
export class Lines {
 readonly child:ChildProcessWithoutNullStreams;
 private buffer='';private lines:string[]=[];private waiter:{resolve:(s:string)=>void;reject:(e:Error)=>void}|undefined;private failure:Error|undefined;
 stderr='';
 constructor(command:string[],cwd:string,env:NodeJS.ProcessEnv=process.env){
  this.child=spawn(command[0]!,command.slice(1),{cwd,env,stdio:'pipe'});
  this.child.stdout.setEncoding('utf8');this.child.stderr.setEncoding('utf8');
  this.child.stdout.on('data',(s:string)=>{this.buffer+=s;if(this.buffer.length>2_000_000){this.fail(new Error('worker output exceeds 2MB'));return;}let i;while((i=this.buffer.indexOf('\n'))>=0){const line=this.buffer.slice(0,i);this.buffer=this.buffer.slice(i+1);if(this.waiter){const w=this.waiter;this.waiter=undefined;w.resolve(line);}else this.lines.push(line);}});
  this.child.stderr.on('data',(s:string)=>{this.stderr=(this.stderr+s).slice(-16000);});
  this.child.on('error',e=>this.fail(e));this.child.on('exit',(code,signal)=>this.fail(new Error(`worker exited ${code}/${signal}: ${this.stderr}`)));
 }
 private fail(error:Error){this.failure=error;this.waiter?.reject(error);this.waiter=undefined;}
 async send(event:unknown):Promise<unknown>{
  if(this.failure)throw this.failure;
  this.child.stdin.write(JSON.stringify(event)+'\n');
  let timer:ReturnType<typeof setTimeout>|undefined;
  try{
   const line=await Promise.race([new Promise<string>((resolve,reject)=>{if(this.lines.length)resolve(this.lines.shift()!);else this.waiter={resolve,reject};}),new Promise<never>((_,reject)=>{timer=setTimeout(()=>reject(new Error('worker response timeout (5s)')),5000);})]);
   return JSON.parse(line);
  }finally{clearTimeout(timer);}
 }
 async close(){if(this.child.exitCode!==null||this.child.signalCode)return;const exited=new Promise<void>(resolve=>this.child.once('exit',()=>resolve()));this.child.kill('SIGKILL');await exited;}
}
export async function command(args:string[],cwd:string,env:NodeJS.ProcessEnv=process.env,timeout=120000){
 const child=spawn(args[0]!,args.slice(1),{cwd,env,stdio:['ignore','pipe','pipe'],detached:true});let stdout='',stderr='';
 child.stdout.on('data',s=>{stdout=(stdout+s).slice(-1_000_000);});child.stderr.on('data',s=>{stderr=(stderr+s).slice(-1_000_000);});
 const timer=setTimeout(()=>{try{process.kill(-child.pid!,'SIGKILL');}catch{}},timeout);
 try{const code=await new Promise<number|null>((resolve,reject)=>{child.on('error',reject);child.on('exit',resolve);});return {code,stdout,stderr};}finally{clearTimeout(timer);}
}
