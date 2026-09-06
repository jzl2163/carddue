#!/usr/bin/env node
import { randomBytes, createECDH } from 'node:crypto';
import { writeFileSync, chmodSync } from 'node:fs';
const args=process.argv.slice(2);
const development=args.includes('--development');
const value=(flag)=>{const i=args.indexOf(flag);return i<0?undefined:args[i+1];};
const destination=value('--output')||'.env';
const domain=value('--domain');
if(!development&&!domain){console.error('Usage: node scripts/init-env.mjs --domain card.example.com [--email admin@example.com] [--output .env]\nLocal development: node scripts/init-env.mjs --development');process.exit(1);}
if(domain&&!/^(?=.{1,253}$)[a-zA-Z0-9](?:[a-zA-Z0-9.-]*[a-zA-Z0-9])?$/.test(domain)){throw new Error('Domain must be a hostname, without scheme, path, port or credentials.');}
const email=value('--email')||'admin@example.com';
if(!/^[^\s@\r\n=]+@[^\s@\r\n=]+$/.test(email)){throw new Error('Invalid VAPID contact email.');}
const key=()=>randomBytes(32).toString('base64url');
const dbPassword=key();
const ecdh=createECDH('prime256v1');ecdh.generateKeys();
const entries={
  APP_ENV:development?'development':'production',
  APP_BASE_URL:development?'http://localhost:8080':`https://${domain}`,
  APP_DOMAIN:domain||'localhost',APP_BIND:'0.0.0.0:8080',APP_PORT:'8080',APP_ROLE:'all',DEFAULT_TIMEZONE:'Asia/Shanghai',
  POSTGRES_USER:'carddue',POSTGRES_DB:'carddue',POSTGRES_PASSWORD:dbPassword,
  DATABASE_URL:`postgres://carddue:${dbPassword}@postgres:5432/carddue`,
  APP_ENCRYPTION_KEY:key(),ICS_SIGNING_KEY:key(),SETUP_TOKEN:key(),ALLOW_REGISTRATION:'false',
  NOTIFICATION_PRIVATE_HOSTS:development?'mailpit,localhost,127.0.0.1':'',ALLOW_INSECURE_NOTIFICATIONS:development?'true':'false',
  VAPID_PRIVATE_KEY:ecdh.getPrivateKey().toString('base64url'),VAPID_SUBJECT:`mailto:${email}`,
  FRONTEND_DIR:'/app/frontend',RUST_LOG:'carddue=info,tower_http=warn,sqlx=warn'
};
const content='# Private CardDue configuration. Never commit this file.\n'+Object.entries(entries).map(([k,v])=>`${k}=${v}`).join('\n')+'\n';
try{writeFileSync(destination,content,{flag:'wx',mode:0o600});chmodSync(destination,0o600);}catch(error){console.error(`Could not create ${destination}: ${error.message}. Existing files are never overwritten.`);process.exit(1);}
console.log(`Created ${destination}. Keep an encrypted backup of it together with PostgreSQL backups.`);
console.log('Read SETUP_TOKEN from this private file during first-run setup; it is intentionally not printed to logs.');
