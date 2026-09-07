<script lang="ts">
  import { Landmark } from '@lucide/svelte';
  let {name='',kind='bank',color='#2563eb'}:{name?:string;kind?:'bank'|'network';color?:string}=$props();
  const key=$derived(name.toLowerCase());
</script>
<span class="brand-mark" style:color={color} aria-hidden="true">
  {#if kind==='bank'}<Landmark size={18}/>
  {:else if key==='mastercard'}<svg viewBox="0 0 32 22" width="28" height="20"><circle cx="11" cy="11" r="9" fill="#eb001b"/><circle cx="21" cy="11" r="9" fill="#f79e1b" fill-opacity=".85"/></svg>
  {:else if key==='visa'}<strong style="color:#28449a;font-style:italic">VISA</strong>
  {:else if key==='银联'||key==='unionpay'}<strong style="color:#087d86">银联</strong>
  {:else if key==='american express'}<strong style="color:#1476ad">AMEX</strong>
  {:else if key==='jcb'}<strong style="color:#197245">JCB</strong>
  {:else}<strong>{name.slice(0,2)||'卡'}</strong>{/if}
</span>
<style>.brand-mark{display:inline-flex;align-items:center;justify-content:center;min-width:32px;height:28px;flex:none;border:1px solid currentColor;border-radius:7px;background:var(--surface);font-size:10px;padding:2px 3px}</style>
