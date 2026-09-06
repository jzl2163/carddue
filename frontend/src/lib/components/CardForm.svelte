<script lang="ts">
  import { untrack } from 'svelte';
  import type { CardInput } from '$lib/types';
  import { defaultCard } from '$lib/defaults';
  import Button from './ui/button/Button.svelte';
  let { initial, onsave, cancelHref = '/cards' }: { initial?: CardInput; onsave: (card: CardInput) => Promise<void>; cancelHref?: string } = $props();
  let card = $state<CardInput>(untrack(() => structuredClone(initial || defaultCard())));
  let annual = $state(untrack(() => Boolean(initial?.annual_fee_month)));
  let busy = $state(false), error = $state('');
  async function submit(event: SubmitEvent) {
    event.preventDefault(); error=''; busy=true;
    try {
      await onsave({ ...card, name: card.name.trim(), due_day: card.due_day || 1, due_offset_days: card.due_offset_days || 20, annual_fee_month: annual ? card.annual_fee_month || 1 : null, annual_fee_day: annual ? card.annual_fee_day || 1 : null, annual_fee_amount: annual && card.annual_fee_amount ? card.annual_fee_amount : null });
    } catch (e) { error = e instanceof Error ? e.message : '保存失败'; } finally { busy=false; }
  }
</script>
<form class="stack" onsubmit={submit}>
  {#if error}<div class="alert error" role="alert">{error}</div>{/if}
  <div class="panel stack"><h2>卡片信息</h2><div class="form-grid">
    <label class="full">卡片名称<input bind:value={card.name} required maxlength="100" placeholder="例如：招商 Visa 日常卡" /></label>
    <label>发卡行<input bind:value={card.issuer} maxlength="100" placeholder="招商银行" /></label>
    <label>卡组织<input bind:value={card.network} maxlength="40" placeholder="Visa / Mastercard / 银联" /></label>
    <label>卡号尾四位<input bind:value={card.last4} inputmode="numeric" pattern="[0-9]{4}|" maxlength="4" autocomplete="off" placeholder="1234" /><small>可留空；不要输入完整卡号。</small></label>
    <label>币种<input bind:value={card.currency} required pattern="[A-Z]{3}" maxlength="3" placeholder="CNY" /></label>
    <label>识别色<input type="color" bind:value={card.color} /></label>
  </div></div>
  <div class="panel stack"><h2>账单与还款规则</h2><div class="form-grid">
    <label>每月账单日<input type="number" min="1" max="31" required bind:value={card.statement_day} disabled={card.statement_last_day} /></label>
    <label class="check"><input type="checkbox" bind:checked={card.statement_last_day} />每月最后一天出账</label>
    <label class="full">还款日计算方式<select bind:value={card.due_mode}><option value="fixed_day">固定日期</option><option value="days_after_statement">账单日之后若干天</option></select></label>
    {#if card.due_mode === 'fixed_day'}
      <label>还款所在月份<select bind:value={card.due_month_offset}><option value={0}>账单同月</option><option value={1}>账单次月</option><option value={2}>账单后第二个月</option></select></label>
      <label>还款日<input type="number" min="1" max="31" required bind:value={card.due_day} disabled={card.due_last_day} /></label>
      <label class="check full"><input type="checkbox" bind:checked={card.due_last_day} />该月最后一天还款</label>
    {:else}<label class="full">账单后几天还款<input type="number" min="1" max="90" required bind:value={card.due_offset_days} /></label>{/if}
    <label class="full">周末调整<select bind:value={card.weekend_adjustment}><option value="none">不调整（推荐以银行账单为准）</option><option value="previous_business_day">遇周末提前到周五</option><option value="next_business_day">遇周末顺延到周一</option></select><small>只处理周六、周日，不包含法定节假日或银行规则。</small></label>
  </div><div class="hint">不存在的日期会截到当月最后一天。例如每月 31 日在二月为 28 日或 29 日。某一期有变化，可在账期中单独修改，不会改动整张卡的规则。</div></div>
  <div class="panel stack"><label class="check"><input type="checkbox" bind:checked={annual} />管理年费日期</label>{#if annual}<div class="form-grid"><label>月份<input type="number" min="1" max="12" bind:value={card.annual_fee_month} required /></label><label>日期<input type="number" min="1" max="31" bind:value={card.annual_fee_day} required /></label><label class="full">年费金额（可选）<input bind:value={card.annual_fee_amount} inputmode="decimal" pattern="[0-9]{1,12}(\.[0-9]{1,2})?" placeholder="例如 100.00" /></label></div>{/if}
  <label>备注<textarea bind:value={card.notes} rows="3" maxlength="4000" placeholder="不要保存完整卡号、CVV、PIN 或银行密码。"></textarea></label></div>
  <div class="form-footer"><a class="button button-outline" href={cancelHref}>取消</a><Button type="submit" disabled={busy}>{busy?'正在保存…':'保存信用卡'}</Button></div>
</form>
