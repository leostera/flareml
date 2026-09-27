import type {Task} from '../tasks/specs';
import type {Request,State} from './reference';
export type Client={worker:number;request:Request;lostAck?:boolean};
export type Case={name:string;seed:number;config:State;batches:Client[][];restartAfter?:number[]};
const client=(worker:number,request:Request,lostAck=false):Client=>({worker,request,lostAck});
const inspect=[client(0,{op:'inspect'}),client(1,{op:'inspect'})];
export function cases(task:Task):Case[]{
 let data:Omit<Case,'seed'>[]=[];
 if(task==='url-shortener'){
  const shorten=(key:string,url:string,candidates=['x','y','z']):Request=>({op:'shorten',key,url,candidates});
  data=[
   {name:'collision-concurrency',config:{},batches:[[client(0,shorten('a','url-A')),client(1,shorten('b','url-B')),client(0,shorten('c','url-C'))],inspect,[client(1,{op:'resolve',code:'x'}),client(0,{op:'resolve',code:'missing'})]]},
   {name:'retry-and-conflict',config:{},batches:[[client(0,shorten('same','url-A'),true)],[client(1,shorten('same','url-A',['q'])),client(0,shorten('same','url-B'))],inspect],restartAfter:[0]},
   {name:'exhaustion-not-cached',config:{},batches:[[client(0,shorten('a','url-A',['x']))],[client(1,shorten('b','url-B',['x']))],[client(0,shorten('b','url-B',['y']))],inspect]},
   {name:'same-url-and-prototype-keys',config:{},batches:[[client(0,shorten('__proto__','https://例.test/🦀',['constructor'])),client(1,shorten('constructor','https://例.test/🦀',['constructor']))],inspect,[client(1,shorten('__proto__','other',['other']))],inspect],restartAfter:[0]},
   {name:'duplicate-inflight',config:{},batches:[[client(0,shorten('a','url-A')),client(1,shorten('a','url-A')),client(0,shorten('a','url-B'))],inspect],restartAfter:[0]},
  ];
 }else if(task==='inventory'){
  const reserve=(id:string,quantity:number):Request=>({op:'reserve',id,quantity});
  data=[
   {name:'oversell',config:{stock:3},batches:[[client(0,reserve('a',2)),client(1,reserve('b',2)),client(0,reserve('c',1))],inspect]},
   {name:'duplicate-reservation',config:{stock:5},batches:[[client(0,reserve('a',3),true),client(1,reserve('a',3))],[client(1,reserve('a',3)),client(0,reserve('a',2))],inspect],restartAfter:[0]},
   {name:'confirm-cancel-race',config:{stock:5},batches:[[client(0,reserve('a',3))],[client(0,{op:'confirm',id:'a'}),client(1,{op:'cancel',id:'a'})],[client(1,{op:'cancel',id:'a'}),client(0,reserve('a',3))],inspect],restartAfter:[1]},
   {name:'release-once-and-retry-insufficient',config:{stock:3},batches:[[client(0,reserve('a',3))],[client(1,reserve('b',2))],[client(0,{op:'cancel',id:'a'}),client(1,{op:'cancel',id:'a'})],[client(1,reserve('b',2)),client(0,reserve('a',3))],inspect]},
   {name:'unicode-prototype-and-missing',config:{stock:5},batches:[[client(0,{op:'cancel',id:'missing'}),client(1,{op:'confirm',id:'missing'})],[client(0,reserve('__proto__',1)),client(1,reserve('予約🦀',2)),client(0,reserve('constructor',1))],inspect],restartAfter:[1]},
  ];
 }else{
  const charge=(key:string,amount:number):Request=>({op:'charge',key,amount});
  data=[
   {name:'overdraft',config:{balance:5},batches:[[client(0,charge('a',3)),client(1,charge('b',3)),client(0,charge('c',2))],inspect]},
   {name:'lost-ack-and-conflict',config:{balance:10},batches:[[client(0,charge('a',4),true)],[client(1,charge('a',4)),client(0,charge('a',3))],inspect],restartAfter:[0]},
   {name:'refund-once',config:{balance:10},batches:[[client(0,charge('a',7))],[client(0,{op:'refund',key:'a'}),client(1,{op:'refund',key:'a'})],[client(1,charge('a',7)),client(0,charge('a',6))],inspect],restartAfter:[1]},
   {name:'insufficient-not-cached',config:{balance:5},batches:[[client(0,charge('a',5))],[client(1,charge('b',3))],[client(0,{op:'refund',key:'a'})],[client(1,charge('b',3)),client(0,charge('a',5))],inspect]},
   {name:'charge-refund-race',config:{balance:8},batches:[[client(0,charge('a',5)),client(1,{op:'refund',key:'a'}),client(1,charge('a',5))],inspect]},
   {name:'unicode-prototype-and-missing',config:{balance:10},batches:[[client(0,{op:'refund',key:'missing'})],[client(0,charge('__proto__',2)),client(1,charge('支払🦀',3)),client(0,charge('constructor',1))],inspect],restartAfter:[1]},
  ];
 }
 // Common frozen schedules for both modes and all languages, not EvalKit's
 // fresh randomSeed. Different storage/response orders exercise the same spec.
 return data.flatMap(c=>[1,17,1729,64007].map(seed=>({...c,name:`${c.name}/seed-${seed}`,seed})));
}
