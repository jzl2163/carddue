<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { regionLabel } from '$lib/card-catalog';
  import BrandIcon from '$lib/components/BrandIcon.svelte';
  import Loading from '$lib/components/Loading.svelte';
  import Empty from '$lib/components/Empty.svelte';
  import type { Ranking } from '$lib/ranking';
  import { rankCards } from '$lib/ranking';
  let data=$state<Ranking|null>(null),error=$state(''),mode=$state<'days'|'longest_days'>('days');
  const ranked=$derived(rankCards(data?.cards||[],mode));
  const maximum=$derived(Math.max(1,...ranked.map(c=>c[mode])));
  async function load(){try{data=await api('/cards/ranking');error='';}catch(e){error=e instanceof Error?e.message:'无法加载';}}
  onMount(()=>{void load();});
</script>
<div class="page-heading"><div><div class="eyebrow">Interest-free days</div><h1>免息期排行榜</h1><p>同一时刻比较，找到还款时间更从容的卡。</p></div><button class="button button-outline" onclick={load}>刷新</button></div>
<div class="tabs" aria-label="排名方式"><button aria-pressed={mode==='days'} class:active={mode==='days'} onclick={()=>mode='days'}>今天消费</button><button aria-pressed={mode==='longest_days'} class:active={mode==='longest_days'} onclick={()=>mode='longest_days'}>未来一年理论最长</button></div>
<div class="hint mb-6">按卡片规则和单期日期覆盖估算，假设消费当天入账、账单日当天计入本期；天数为还款日期减消费日期。实际入账、是否享有免息及银行宽限规则可能不同。理论最长按上一账单日次日消费估算。</div>
{#if error}<div class="alert error" role="alert">{error}</div>{:else if !data}<Loading/>{:else if !ranked.length}<div class="panel"><Empty title="添加卡片后开始比较" description="排行榜只包含未归档的信用卡。"/><a class="button button-default" href="/cards/new">添加信用卡</a></div>{:else}
<div class="ranking-list">{#each ranked as c,i}<a class="panel rank-card" href={'/cards/'+c.id}>
  <div class="rank-top"><span class="rank-number">{i+1}</span><BrandIcon name={c.issuer} color={c.color}/><div class="grow"><h2>{c.name}</h2><p class="text-xs">{c.issuer||'未填写发卡行'} · {regionLabel(c.region)}</p></div><BrandIcon name={c.network} kind="network" color={c.color}/><div class="rank-days">{c[mode]}<small> 天</small></div></div>
  <div class="rank-bar"><span style:width={Math.max(2,c[mode]/maximum*100)+'%'} style:background={c.color}></span></div>
  <div class="rank-meta"><span>当地今天 {c.local_today}</span><span>预计入账账单 {c.statement_date}</span><span>对应还款日 {c.due_date}</span></div>
  {#if mode==='longest_days'}<p class="text-xs mt-2">今天消费预计 {c.days} 天；上方日期为今天消费对应账期。</p>{/if}
  {#if c.different_timezone}<p class="timezone-note">◷ 卡片时区 {c.timezone}，与账户 {data.account_timezone} 不同{c.different_date?'，当地日期也不同':''}。按卡片当地日期计算。</p>{/if}
</a>{/each}</div>
<p class="text-xs mt-4 muted">计算时间：{new Date(data.as_of).toLocaleString()} · 跨午夜后请刷新。</p>{/if}
<style>.ranking-list{display:grid;gap:16px}.rank-top{display:flex;align-items:center;gap:12px}.rank-top h2{font-size:17px}.rank-number{font-size:20px;font-weight:700;color:var(--muted);width:25px}.rank-days{font-size:36px;font-weight:750;white-space:nowrap;letter-spacing:-1px}.rank-days small{font-size:12px;letter-spacing:0}.rank-bar{height:7px;background:var(--line);border-radius:10px;margin:18px 0;overflow:hidden}.rank-bar span{display:block;height:100%;border-radius:10px}.rank-meta{display:flex;flex-wrap:wrap;gap:10px 24px;font-size:11px;color:var(--muted)}.timezone-note{font-size:11px;color:var(--accent);margin-top:12px;padding:9px 12px;background:var(--accent-soft);border-radius:8px}@media(max-width:500px){.rank-top{gap:8px;flex-wrap:wrap}.rank-top h2{font-size:15px}.rank-top>.grow{min-width:130px}.rank-days{font-size:30px}}</style>
