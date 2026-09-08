<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import type { Ranking } from '$lib/ranking';
  import { rankCards } from '$lib/ranking';
  import BrandIcon from './BrandIcon.svelte';
  let data=$state<Ranking|null>(null),error=$state(''),loading=$state(true);
  const top=$derived(rankCards(data?.cards||[]).slice(0,3));
  async function load(){
    loading=true;error='';
    try{data=await api<Ranking>('/cards/ranking');}
    catch(e){error=e instanceof Error?e.message:'无法加载免息排行榜';}
    finally{loading=false;}
  }
  onMount(()=>{void load();});
</script>
<section class="panel mini-ranking" aria-labelledby="mini-ranking-title">
  <div class="panel-heading"><div><h2 id="mini-ranking-title">免息排行榜</h2><p>今天消费 · 预计免息天数前三名</p></div><a class="subtle-link" href="/ranking">查看全部 →</a></div>
  {#if loading}<p role="status">正在计算免息天数…</p>
  {:else if error}<div role="alert"><p>{error}</p><button class="button button-outline" onclick={load}>重试</button></div>
  {:else if !top.length}<p>添加信用卡后，即可比较今天消费的预计免息天数。</p><a class="subtle-link" href="/cards/new">添加信用卡 →</a>
  {:else}
    <ol>{#each top as card,i}<li><a href={'/cards/'+card.id}>
      <span class="position">{i+1}</span><BrandIcon name={card.issuer} color={card.color}/>
      <div class="card-info"><h3>{card.name}</h3><p>还款日 {card.due_date}</p>{#if card.different_timezone}<small>按 {card.timezone} · {card.local_today} 计算</small>{/if}</div>
      <strong>{card.days}<small> 天</small></strong>
    </a></li>{/each}</ol>
    <p class="ranking-hint">假设今天消费当天入账，实际免息期以银行规则为准。</p>
    <div class="ranking-footer"><small>更新于 {data?new Date(data.as_of).toLocaleString():''}</small><button class="subtle-link" onclick={load}>刷新</button></div>
  {/if}
</section>
<style>
  .panel-heading{margin-bottom:10px;flex-wrap:wrap}
  .panel-heading p,.mini-ranking>p{font-size:12px}
  .panel-heading .subtle-link{white-space:nowrap}
  ol{list-style:none;padding:0;margin:0;display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:16px}
  li a{display:flex;align-items:center;gap:10px;padding:12px 0;border-radius:8px}
  li a:hover{background:var(--accent-soft)}
  .position{font-size:16px;font-weight:700;color:var(--muted)}
  .card-info{flex:1;min-width:0}
  h3{font-size:13px;overflow-wrap:anywhere}
  .card-info p,.card-info small{font-size:10px;color:var(--muted)}
  strong{font-size:26px;white-space:nowrap}
  strong small{font-size:11px;font-weight:400}
  .ranking-hint{margin-top:8px;color:var(--muted)}
  .ranking-footer{display:flex;justify-content:space-between;gap:12px;margin-top:8px;color:var(--muted)}
  .ranking-footer small{font-size:10px}
  @media(max-width:1100px){ol{grid-template-columns:1fr;gap:0}li+li{border-top:1px solid var(--line)}}
</style>
