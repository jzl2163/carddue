#!/usr/bin/env node
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { gzipSync } from 'node:zlib';
const root=resolve(import.meta.dirname,'../frontend/build');
const walk=(dir)=>readdirSync(dir).flatMap(name=>{const path=join(dir,name);return statSync(path).isDirectory()?walk(path):[path];});
let js=0,css=0;
for(const path of walk(root)){
  if(path.endsWith('.js'))js+=gzipSync(readFileSync(path)).length;
  if(path.endsWith('.css'))css+=gzipSync(readFileSync(path)).length;
}
// This conservative budget counts every route, not only initial navigation.
console.log(JSON.stringify({scope:'all static routes, individually gzipped',javascript_bytes:js,css_bytes:css,limits:{javascript:200*1024,css:50*1024}},null,2));
if(js>200*1024||css>50*1024){console.error('Static bundle exceeds the documented budget. Inspect imports and code splitting instead of silently increasing the limits.');process.exit(1);}
