import {ensureTla,tlaJar,tlaSha} from '../lib/tla';
await ensureTla();
console.log(`TLC ready: ${tlaJar()}\nSHA-256: ${tlaSha}`);
