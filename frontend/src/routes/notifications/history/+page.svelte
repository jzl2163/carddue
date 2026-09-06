<script lang="ts">
  import { onMount } from 'svelte';
  import { api,showError,notify } from '$lib/api';
  import type { Delivery,Rendered } from '$lib/types';
  import { statusLabels,channelLabels } from '$lib/utils';
  import Button from '$lib/components/ui/button/Button.svelte';
  import Loading from '$lib/components/Loading.svelte';
  import Empty from '$lib/components/Empty.svelte';
  interface Attempt{started_at:string;finished_at:string;success:boolean;status:number|null;error_code:string|null;rendered:Rendered|null}
  let rows=$state<Delivery[]|null>(null),offset=$state(0),error=$state(''),attempts=$state<Attempt[]|null>(null),selected=$state(''),busy=$state(false);
  async function load(){busy=true;try{rows=await api<Delivery[]>(`/notification/deliveries?offset=${offset}`);error='';}catch(e){error=e instanceof Error?e.message:'读取失败';}finally{busy=false;}}
  onMount(()=>{void load();});
  async function details(id:string){selected=id;try{attempts=await api<Attempt[]>(`/notification/deliveries/${id}/attempts`);}catch(e){showError(e);}}
  async function retry(id:string){try{await api(`/notification/deliveries/${id}/retry`,'POST');await load();notify('已申请重试；任务仍会检查已还款、停用和过期状态。');}catch(e){showError(e);}}
  function move(delta:number){offset=Math.max(0,offset+delta);void load();}
</script>
<div class="stack"><div class="panel-heading"><div><h2>通知投递记录</h2><small>“服务商已接收”不是设备已显示或用户已阅读的保证。</small></div><Button variant="outline" disabled={busy} onclick={load}>刷新</Button></div>
{#if error}<div class="alert error" role="alert">{error}</div>{:else if rows===null}<Loading/>{:else if !rows.length}<div class="panel"><Empty title="这里还没有投递记录" description="发送一条测试消息，或创建提醒规则后，会在这里显示队列状态与发送结果。"/></div>{:else}<div class="panel table-wrap"><table><thead><tr><th>消息</th><th>渠道</th><th>计划时间</th><th>状态</th><th>尝试</th><th>操作</th></tr></thead><tbody>{#each rows as r}<tr><td><strong>{r.context.card.name}</strong><div class="muted">{r.context.event.label} · {r.context.event.date}</div></td><td>{r.connection}<div class="muted">{channelLabels[r.kind]}</div></td><td>{new Date(r.scheduled_at).toLocaleString()}</td><td><span class="badge" class:success={r.status==='sent'} class:danger={r.status==='dead'}>{statusLabels[r.status]||r.status}</span>{#if r.error}<div class="muted mono text-xs mt-1">{r.error}</div>{/if}</td><td>{r.attempt_count}</td><td><div class="actions"><Button variant="outline" onclick={()=>details(r.id)}>详情</Button>{#if r.status==='dead'}<Button variant="ghost" onclick={()=>retry(r.id)}>重试</Button>{/if}</div></td></tr>{/each}</tbody></table></div>{/if}
<div class="actions"><Button variant="outline" disabled={offset===0||busy} onclick={()=>move(-100)}>上一页</Button><span class="muted text-xs">第 {offset/100+1} 页</span><Button variant="outline" disabled={!rows||rows.length<100||busy} onclick={()=>move(100)}>下一页</Button></div>
{#if attempts}<section class="panel stack"><div class="panel-heading"><h2>发送尝试</h2><Button variant="ghost" onclick={()=>attempts=null}>关闭</Button></div><small class="mono muted break-all">任务 {selected}</small>{#each attempts as a}<div class="stack"><div class="actions"><span class="badge" class:success={a.success}>{a.success?'成功':'失败'}</span><small>{new Date(a.finished_at).toLocaleString()}</small><code class="text-xs">{a.error_code||a.status||''}</code></div>{#if a.rendered}<h3>{a.rendered.title}</h3><pre>{a.rendered.body}</pre>{/if}</div>{:else}<p class="text-sm">任务尚未发送，或详细记录已超过保留期限。</p>{/each}</section>{/if}
<div class="hint">暂时性网络错误、限流或服务商故障会按退避策略重试，最多 5 次。任务过期后不会继续补发。极端崩溃场景可能重复送达；不要在 Webhook 中直接执行还款等不可逆操作。</div></div>
