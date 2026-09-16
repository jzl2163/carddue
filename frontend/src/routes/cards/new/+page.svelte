<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import { api, notify } from '$lib/api';
  import type { Card, CardInput } from '$lib/types';
  import CardForm from '$lib/components/CardForm.svelte';
  import Loading from '$lib/components/Loading.svelte';
  let cards=$state<Card[]>([]),selected=$state(''),initial=$state<CardInput|undefined>(undefined),version=$state(0),loading=$state(true),error=$state('');
  function useCard(id:string){
    const source=cards.find(c=>c.id===id);
    selected=id;
    initial=source?{...$state.snapshot(source.data),name:source.data.name.slice(0,94)+'（副本）',last4:''}:undefined;
    version+=1;
  }
  function changeSource(event:Event){
    const select=event.currentTarget as HTMLSelectElement;
    if(!confirm('切换来源会替换当前表单中尚未保存的信息，继续吗？')){select.value=selected;return;}
    useCard(select.value);
  }
  async function load(){
    loading=true;error='';
    try{
      cards=await api<Card[]>('/cards');
      const source=page.url.searchParams.get('copyFrom')||'';
      if(source&&!cards.some(c=>c.id===source)){error='来源卡片不存在或无法访问，请选择其他卡片或从空白开始。';useCard('');}
      else useCard(source);
    }catch(e){error=e instanceof Error?e.message:'读取卡片失败';}
    finally{loading=false;}
  }
  onMount(()=>{void load();});
  async function save(card:CardInput){const result=await api<{id:string}>('/cards','POST',card);notify('信用卡已添加，账期已生成。');await goto(`/cards/${result.id}`);}
</script>
<div class="page-heading"><div><div class="eyebrow">New card</div><h1>添加信用卡</h1><p>先建立规则，再核对本期银行实际账单。</p></div></div>
{#if loading}<Loading/>{:else}
{#if error}<div class="alert error mb-4" role="alert">{error} <button type="button" class="subtle-link" onclick={load}>重新读取</button></div>{/if}
<section class="panel stack mb-6"><label>复用已有信用卡信息<select value={selected} onchange={changeSource}><option value="">从空白开始</option>{#each cards as card}<option value={card.id}>{card.data.name}{card.data.last4?' · '+card.data.last4:''}{card.active?'':'（已归档）'}</option>{/each}</select></label><p class="text-xs muted">复用发卡行、卡组织、地区、时区、账单与还款规则、年费设置及备注。新卡尾号留空，请核对名称和差异后保存。账单金额、还款记录、权益事项及单独配置的提醒规则不会复制。</p>{#if selected}<div class="hint">当前是新卡草稿，保存后创建独立信用卡，原卡保持不变。</div>{/if}</section>
{#key version}<CardForm {initial} onsave={save}/>{/key}
{/if}
