<script lang="ts">
  import { goto } from '$app/navigation';
  import { api, notify } from '$lib/api';
  import type { CardInput } from '$lib/types';
  import CardForm from '$lib/components/CardForm.svelte';
  async function save(card:CardInput){const result=await api<{id:string}>('/cards','POST',card);notify('信用卡已添加，账期已生成。');await goto(`/cards/${result.id}`);}
</script>
<div class="page-heading"><div><div class="eyebrow">New card</div><h1>添加信用卡</h1><p>先建立规则，再核对本期银行实际账单。</p></div></div>
<CardForm onsave={save}/>
