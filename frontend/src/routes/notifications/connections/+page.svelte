<script lang="ts">
  import { onMount } from 'svelte';
  import { BellRing,Send,Plus } from '@lucide/svelte';
  import { api,notify,showError } from '$lib/api';
  import type { Connection } from '$lib/types';
  import { channelLabels } from '$lib/utils';
  import Button from '$lib/components/ui/button/Button.svelte';
  import Empty from '$lib/components/Empty.svelte';
  import Loading from '$lib/components/Loading.svelte';
  let rows=$state<Connection[]|null>(null),error=$state(''),kind=$state('bark'),name=$state(''),id=$state<string|null>(null),editing=$state(false),replace=$state(true),enabled=$state(true),busy=$state('');
  let config=$state<Record<string,string>>({base_url:'https://api.day.app',device_key:'',host:'',port:'465',security:'tls',username:'',password:'',from:'',to:'',bot_token:'',chat_id:'',url:'',bearer_token:''});
  let vapid=$state<{enabled:boolean;public_key:string|null}>({enabled:false,public_key:null});
  let browserId=$state(''),browserSupported=$state(false);
  async function load(){try{[rows,vapid]=await Promise.all([api<Connection[]>('/notification/connections'),api<typeof vapid>('/notification/webpush/key')]);}catch(e){error=e instanceof Error?e.message:'读取失败';}}
  onMount(()=>{browserSupported=window.isSecureContext&&'serviceWorker'in navigator&&'PushManager'in window&&'Notification'in window;browserId=localStorage.getItem('carddue-browser-connection')||'';void load();});
  function open(row?:Connection){editing=true;id=row?.id||null;name=row?.name||'';kind=row?.kind||'bark';enabled=row?.enabled??true;replace=!row;config={base_url:'https://api.day.app',device_key:'',host:'',port:'465',security:'tls',username:'',password:'',from:'',to:'',bot_token:'',chat_id:'',url:'',bearer_token:''};}
  function payload():Record<string,unknown>{switch(kind){case'bark':return{base_url:config.base_url,device_key:config.device_key};case'email':return{host:config.host,port:Number(config.port),security:config.security,username:config.username,password:config.password,from:config.from,to:config.to};case'telegram':return{bot_token:config.bot_token,chat_id:config.chat_id};case'webhook':return{url:config.url,bearer_token:config.bearer_token};default:throw new Error('浏览器推送请使用专用按钮重新绑定。');}}
  async function save(event:SubmitEvent){event.preventDefault();busy='save';try{await api(`/notification/connections${id?'/'+id:''}`,id?'PATCH':'POST',{name,kind,enabled,config:replace?payload():null});editing=false;config={};await load();notify('通知渠道已保存。请发送测试消息并检查投递记录。');}catch(e){showError(e);}finally{busy='';}}
  async function test(row:Connection){busy=row.id;try{await api(`/notification/connections/${row.id}/test`,'POST',{});notify('测试通知已加入队列，请到“投递记录”检查结果。');}catch(e){showError(e);}finally{busy='';}}
  async function disable(row:Connection){if(!confirm(`停用“${row.name}”？该渠道未发送的通知将取消。`))return;try{await api(`/notification/connections/${row.id}`,'DELETE');await load();}catch(e){showError(e);}}
  function keyBytes(key:string):Uint8Array<ArrayBuffer>{const value=atob(key.replace(/-/g,'+').replace(/_/g,'/')+'='.repeat((4-key.length%4)%4));return Uint8Array.from(value,c=>c.charCodeAt(0));}
  async function enableBrowser(){
    if(!browserSupported||!vapid.public_key)return;
    // Invoke the permission prompt directly from the user's click, before asynchronous registration.
    const permissionPromise=Notification.requestPermission();busy='webpush';
    try{
      const permission=await permissionPromise;if(permission!=='granted')throw new Error('通知权限未授予。请在浏览器设置中允许本站通知，然后重试。');
      await navigator.serviceWorker.register('/sw.js',{scope:'/'});
      const registration=await navigator.serviceWorker.ready;
      let subscription=await registration.pushManager.getSubscription();
      if(subscription){const existing=subscription.options.applicationServerKey;const desired=keyBytes(vapid.public_key);if(existing&&Array.from(new Uint8Array(existing)).join(',')!==Array.from(desired).join(',')){await subscription.unsubscribe();subscription=null;}}
      subscription=subscription||await registration.pushManager.subscribe({userVisibleOnly:true,applicationServerKey:keyBytes(vapid.public_key)});
      const previous=rows?.find(r=>r.id===browserId&&r.kind==='webpush');
      const result=await api<{id:string}>(`/notification/connections${previous?'/'+previous.id:''}`,previous?'PATCH':'POST',{name:previous?.name||`当前浏览器 · ${new Date().toLocaleDateString()}`,kind:'webpush',enabled:true,config:subscription.toJSON()});
      browserId=result.id;localStorage.setItem('carddue-browser-connection',browserId);await load();notify('浏览器推送已绑定。请将该渠道加入提醒规则。');
    }catch(e){showError(e);}finally{busy='';}
  }
  async function unsubscribe(){if(!confirm('停止当前浏览器的通知订阅？'))return;busy='webpush';try{if(browserId&&rows?.some(r=>r.id===browserId))await api(`/notification/connections/${browserId}`,'DELETE');const registration=await navigator.serviceWorker.getRegistration('/');await(await registration?.pushManager.getSubscription())?.unsubscribe();localStorage.removeItem('carddue-browser-connection');browserId='';await load();notify('当前浏览器已取消订阅。');}catch(e){showError(e);}finally{busy='';}}
</script>
<div class="stack"><section class="panel"><div class="panel-heading"><div class="actions"><BellRing size={22}/><h2>当前浏览器推送</h2></div><span class="badge" class:success={browserSupported&&vapid.enabled}>{browserSupported&&vapid.enabled?'可配置':'暂不可用'}</span></div><p class="text-xs">无需安装独立客户端。订阅信息会加密保存，通知中显示的信息由你的消息模板决定。</p><div class="actions mt-4"><Button disabled={!browserSupported||!vapid.enabled||!!busy} onclick={enableBrowser}>{browserId?'重新绑定当前浏览器':'允许并绑定浏览器通知'}</Button>{#if browserId}<Button variant="outline" disabled={!!busy} onclick={unsubscribe}>取消本机订阅</Button>{/if}</div>{#if !vapid.enabled}<small class="muted block mt-3">管理员需要设置 VAPID_PRIVATE_KEY 和 VAPID_SUBJECT。</small>{/if}{#if !browserSupported}<small class="muted block mt-3">需要支持 Web Push 的浏览器和 HTTPS（本机 localhost 开发除外）。iPhone/iPad 通常需要将本站添加到主屏幕后，从该图标打开。</small>{/if}</section>
<div class="panel-heading"><h2>全部通知渠道</h2><Button onclick={()=>open()}><Plus size={15}/>添加渠道</Button></div>
{#if editing}<form class="panel stack" onsubmit={save}><div class="panel-heading"><h2>{id?'编辑渠道':'新渠道'}</h2><Button variant="ghost" onclick={()=>editing=false}>关闭</Button></div><div class="form-grid"><label>名称<input bind:value={name} required maxlength="100" placeholder="例如：我的 iPhone"/></label><label>渠道类型<select bind:value={kind} disabled={!!id}><option value="bark">Bark</option><option value="email">电子邮件（SMTP）</option><option value="telegram">Telegram</option><option value="webhook">Webhook</option>{#if kind==='webpush'}<option value="webpush">浏览器推送</option>{/if}</select></label><label class="check"><input type="checkbox" bind:checked={enabled}/>启用渠道</label>{#if id&&kind!=='webpush'}<label class="check"><input type="checkbox" bind:checked={replace}/>替换全部连接配置</label>{/if}
{#if replace&&kind!=='webpush'}
{#if kind==='bark'}<label class="full">Bark 服务地址<input bind:value={config.base_url} type="url" required/></label><label class="full">Device Key<input bind:value={config.device_key} type="password" autocomplete="new-password" required/><small>仅填写设备 Key，不要粘贴含 Key 的完整推送 URL。</small></label>
{:else if kind==='email'}<label>SMTP 主机<input bind:value={config.host} required placeholder="smtp.example.com"/></label><label>端口<input bind:value={config.port} inputmode="numeric" required pattern="[0-9]{1,5}"/></label><label class="full">传输安全<select bind:value={config.security}><option value="tls">直接 TLS（常用 465）</option><option value="starttls">强制 STARTTLS（常用 587）</option><option value="none">无加密（仅管理员明确允许的测试主机）</option></select></label><label>用户名<input bind:value={config.username} autocomplete="off"/></label><label>SMTP 密码 / 授权码<input bind:value={config.password} type="password" autocomplete="new-password"/></label><label>发件人<input bind:value={config.from} required placeholder="CardDue <notice@example.com>"/></label><label>收件人<input bind:value={config.to} required placeholder="me@example.com"/></label>
{:else if kind==='telegram'}<label class="full">Bot Token<input bind:value={config.bot_token} type="password" autocomplete="new-password" required/></label><label class="full">Chat ID<input bind:value={config.chat_id} required placeholder="例如 123456789 或 -100…"/><small>先在 Telegram 中向机器人发送 /start。群组应先加入机器人，并确认它有发送权限。</small></label>
{:else if kind==='webhook'}<label class="full">Webhook URL<input bind:value={config.url} type="url" required placeholder="https://example.com/notifications"/></label><label class="full">Bearer Token（可选）<input bind:value={config.bearer_token} type="password" autocomplete="new-password"/></label>{/if}
{:else if id}<div class="hint full">现有凭据不会返回浏览器。只改名称或启用状态会保留原配置；替换配置时需重新填写所有字段。</div>{/if}</div><div class="form-footer"><Button type="submit" disabled={busy==='save'}>{busy==='save'?'正在保存…':'保存渠道'}</Button></div></form>{/if}
{#if error}<div class="alert error" role="alert">{error}</div>{:else if rows===null}<Loading/>{:else if !rows.length}<div class="panel"><Empty title="连接你的第一个通知渠道" description="支持 Bark、邮件、Telegram、浏览器推送与通用 Webhook。"/></div>{:else}{#each rows as row}<section class="panel"><div class="panel-heading"><div><h2>{row.name}</h2><p class="text-xs mt-2">{channelLabels[row.kind]}</p></div><span class="badge" class:success={row.enabled}>{row.enabled?'已启用':'已停用'}</span></div><div class="actions"><Button variant="outline" disabled={!row.enabled||busy===row.id} onclick={()=>test(row)}><Send size={14}/>发送测试</Button><Button variant="ghost" onclick={()=>open(row)}>编辑</Button>{#if row.enabled}<Button variant="ghost" onclick={()=>disable(row)}>停用</Button>{/if}</div></section>{/each}{/if}
<div class="hint">“服务商已接收”不等于用户已阅读。手机静音、系统通知权限、网络和服务商状态都可能影响最终呈现。不要将系统作为唯一还款保障。</div></div>
