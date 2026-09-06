<script lang="ts">
  import { page } from '$app/state';
  let { children } = $props();
  const tabs=[['/notifications','提醒规则'],['/notifications/connections','通知渠道'],['/notifications/templates','消息模板'],['/notifications/history','投递记录']];
</script>
<div class="page-heading"><div><div class="eyebrow">Notification center</div><h1>以你的方式提醒</h1><p>配置渠道、编写模板，再选择何时发送。</p></div></div>
<nav class="tabs" aria-label="通知中心导航">{#each tabs as [href,label]}<a {href} class:active={page.url.pathname===href}>{label}</a>{/each}</nav>
{@render children()}
