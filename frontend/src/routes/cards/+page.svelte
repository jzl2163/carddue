<script lang="ts">
  import { onMount } from 'svelte';
  import { Plus, CreditCard } from '@lucide/svelte';
  import { api } from '$lib/api';
  import type { Card } from '$lib/types';
  import Loading from '$lib/components/Loading.svelte';
  import Empty from '$lib/components/Empty.svelte';
  let cards=$state<Card[]|null>(null),search=$state(''),showArchived=$state(false),error=$state('');
  const filtered=$derived((cards||[]).filter(c=>(showArchived||c.active)&&`${c.data.name} ${c.data.issuer} ${c.data.last4}`.toLowerCase().includes(search.toLowerCase())));
  onMount(()=>{api<Card[]>('/cards').then(v=>cards=v).catch(e=>error=e.message);});
</script>
<div class="page-heading"><div><div class="eyebrow">Your cards</div><h1>信用卡</h1><p>管理日期，不连接银行，也不保存完整卡号。</p></div><a class="button button-default" href="/cards/new"><Plus size={16}/>添加信用卡</a></div>
<div class="actions mb-6"><label class="search grow"><span class="sr-only">搜索卡片</span><input type="search" bind:value={search} placeholder="搜索名称、发卡行或尾号…" /></label><label class="check"><input type="checkbox" bind:checked={showArchived}/>显示已归档</label></div>
{#if error}<div class="alert error" role="alert">{error}</div>{:else if cards===null}<Loading/>{:else if filtered.length===0}<div class="panel"><Empty title={cards.length?'没有匹配的信用卡':'添加你的第一张信用卡'} description="只需填写名称、账单日与还款日，即可建立自己的提醒日程。"/></div>{:else}<div class="card-grid">{#each filtered as c}<a class="credit-card" href={`/cards/${c.id}`} style={`--card-color:${c.data.color}`}><div class="card-top"><CreditCard size={23}/><span class="badge">{c.active?c.data.network||'信用卡':'已归档'}</span></div><h2>{c.data.name}</h2><p class="mt-1 text-xs">{c.data.issuer||'未填写发卡行'}</p><div class="last4">•••• {c.data.last4||'— — — —'}</div><div class="card-facts"><div><span>账单日</span>{c.data.statement_last_day?'每月月末':`每月 ${c.data.statement_day} 日`}</div><div><span>还款日</span>{c.data.due_mode==='fixed_day'?`${['同月','次月','后第二个月'][c.data.due_month_offset||0]} ${c.data.due_last_day?'月末':c.data.due_day+' 日'}`:`出账后 ${c.data.due_offset_days} 天`}</div></div></a>{/each}</div>{/if}
