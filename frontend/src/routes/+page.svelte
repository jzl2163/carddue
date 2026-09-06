<script lang="ts">
  import { onMount } from 'svelte';
  import { Plus, CreditCard, Clock3, CircleCheck, ArrowUpRight } from '@lucide/svelte';
  import { api, showError, notify } from '$lib/api';
  import { daysBetween, dateLabel, kindLabels } from '$lib/utils';
  import type { Card, Dashboard, Upcoming } from '$lib/types';
  import Loading from '$lib/components/Loading.svelte';
  import Empty from '$lib/components/Empty.svelte';
  import Button from '$lib/components/ui/button/Button.svelte';
  let data=$state<Dashboard|null>(null),cards=$state<Card[]>([]),error=$state(''),busy=$state('');
  const today=$derived(data?.today || '');
  const due=$derived(data?.events.filter(e=>e.kind==='payment_due'&&!e.paid) || []);
  const next=$derived(due.find(e=>data&&e.date>=today));
  const overdue=$derived(due.filter(e=>data&&e.date<today));
  const upcoming=$derived(data?.events.filter(e=>e.date>=today&&!e.paid).slice(0,8)||[]);
  async function load(){try{const [d,c]=await Promise.all([api<Dashboard>('/dashboard'),api<Card[]>('/cards')]);data=d;cards=c;error='';}catch(e){error=e instanceof Error?e.message:'加载失败';}}
  onMount(()=>{void load();});
  async function paid(e:Upcoming){if(!e.cycle_id)return;busy=e.id;try{await api(`/cards/${e.card_id}/cycles/${e.cycle_id}/pay`,'POST');notify('本期已标记还款，尚未发送的还款提醒已取消。');await load();}catch(err){showError(err);}finally{busy='';}}
</script>
<div class="page-heading"><div><div class="eyebrow">Your financial calendar</div><h1>每个日期，都从容应对</h1><p>{data?`${dateLabel(today)} · 你的信用卡日程概览`:'集中查看下一步需要处理的事情'}</p></div><a class="button button-default" href="/cards/new"><Plus size={16}/>添加信用卡</a></div>
{#if error}<div class="alert error" role="alert">{error}<Button variant="outline" onclick={load}>重试</Button></div>{:else if !data}<Loading/>{:else}
<div class="stack">
<div class="grid-3"><div class="panel stat"><div class="stat-label">正在管理<CreditCard size={16}/></div><div class="stat-number">{cards.filter(c=>c.active).length}<small> 张</small></div><div class="stat-caption">有效信用卡</div></div><div class="panel stat"><div class="stat-label">未来 30 天<Clock3 size={16}/></div><div class="stat-number">{due.filter(e=>daysBetween(e.date,today)>=0&&daysBetween(e.date,today)<=30).length}<small> 笔</small></div><div class="stat-caption">待处理还款日</div></div><div class="panel stat"><div class="stat-label">已完成<CircleCheck size={16}/></div><div class="stat-number">{data.events.filter(e=>e.kind==='payment_due'&&e.paid&&e.date.slice(0,7)===today.slice(0,7)).length}<small> 笔</small></div><div class="stat-caption">本月手工标记已还款</div></div></div>
{#if overdue.length}<div class="alert error"><strong>{overdue.length} 笔日期已过，但尚未标记还款。</strong> 请先核对银行实际状态。{#each overdue as e}<div class="event-row"><span>{e.card_name} · {e.date}</span><Button variant="outline" disabled={busy===e.id} onclick={()=>paid(e)}>标记已还款</Button></div>{/each}</div>{/if}
<div class="grid-2"><section class="panel hero"><div class="eyebrow">Next payment</div>{#if next}<small>下一个还款日</small><h2>{next.card_name}</h2><p>{dateLabel(next.date)}</p><div class="hero-bottom"><div><span class="number">{daysBetween(next.date,today)}</span><small> 天后到期</small></div><Button disabled={busy===next.id} onclick={()=>paid(next)}>标记已还款<CircleCheck size={16}/></Button></div>{:else}<h2>暂时没有待处理还款</h2><p>添加信用卡后，你的下一个重要日期会显示在这里。</p><div class="hero-bottom"><a class="button" href="/cards/new">添加第一张卡<Plus size={16}/></a></div>{/if}</section>
<section class="panel"><div class="panel-heading"><h2>即将到来</h2><a class="subtle-link" href="/calendar">订阅日历<ArrowUpRight size={13} class="inline"/></a></div>{#each upcoming.slice(0,4) as e}<div class="event-row"><div class="event-main"><div class="date-tile">{Number(e.date.slice(8))}<small>{Number(e.date.slice(5,7))} 月</small></div><div><h3>{e.card_name}</h3><p>{kindLabels[e.kind]||e.title}</p></div></div><span class="badge" class:accent={e.kind==='payment_due'}>{daysBetween(e.date,today)===0?'今天':`${daysBetween(e.date,today)} 天后`}</span></div>{:else}<Empty title="日程清晰，暂时无事" description="账单、还款和权益事项会自动汇总在这里。"/>{/each}</section></div>
<section><div class="panel-heading"><h2>我的信用卡</h2><a class="subtle-link" href="/cards">查看全部 →</a></div><div class="card-grid">{#each cards.filter(c=>c.active).slice(0,6) as c}<a class="credit-card" href={`/cards/${c.id}`} style={`--card-color:${c.data.color}`}><div class="card-top"><CreditCard size={22}/><span class="badge">{c.data.network||'信用卡'}</span></div><h2>{c.data.name}</h2><div class="last4">•••• {c.data.last4||'— — — —'}</div><div class="card-facts"><div><span>账单日</span>{c.data.statement_last_day?'月末':`每月 ${c.data.statement_day} 日`}</div><div><span>还款规则</span>{c.data.due_mode==='fixed_day'?`${c.data.due_month_offset?'次期':'本期'} ${c.data.due_last_day?'月末':`${c.data.due_day} 日`}`:`账单后 ${c.data.due_offset_days} 天`}</div></div></a>{/each}</div></section>
</div>{/if}
