<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { timezones } from '$lib/card-catalog';
  import { goto } from '$app/navigation';
  import { CreditCard, ArrowRight, ShieldCheck } from '@lucide/svelte';
  import { api, session } from '$lib/api';
  import type { Session } from '$lib/types';
  import Button from '$lib/components/ui/button/Button.svelte';
  let status = $state<{setup_required:boolean;registration_allowed:boolean;default_timezone:string} | null>(null);
  let email = $state(''), password = $state(''), token = $state(''), timezone = $state('Asia/Shanghai'), error = $state(''), busy = $state(false), register = $state(false), confirmation = $state('');
  onMount(() => { api<typeof status>('/auth/status').then(v => {status=v;register=Boolean(v?.registration_allowed && !v?.setup_required && page.url.searchParams.get('mode')==='register');timezone=v?.default_timezone || 'Asia/Shanghai';}).catch(e=>error=e.message); });
  async function submit(event: SubmitEvent) {
    event.preventDefault(); busy=true; error='';
    try {
      const setup=status?.setup_required;
      if((setup||register)&&password!==confirmation)throw new Error('两次输入的密码不一致');

      const result=await api<Session>(setup?'/auth/setup':register?'/auth/register':'/auth/login','POST',setup||register?{email,password,timezone,setup_token:setup?token:null}:{email,password});
      password='';confirmation='';token='';session.set(result);await goto('/');
    } catch(e) {error=e instanceof Error?e.message:'无法登录';} finally {busy=false;}
  }
</script>
<div class="panel auth-card"><div class="brand"><span class="brand-icon"><CreditCard size={22}/></span><span>CardDue<small>YOUR DATES, IN ONE PLACE</small></span></div>
<h1>{status?.setup_required?'建立你的私有工作空间':register?'创建账户':'欢迎回来'}</h1><p>集中管理信用卡日期，让提醒按你的方式到达。</p>
{#if error}<div class="alert error" role="alert">{error}</div>{/if}
{#if status}<form onsubmit={submit}>
<label>邮箱<input type="email" bind:value={email} autocomplete="username" required maxlength="254" /></label>
<label>密码<input type="password" bind:value={password} autocomplete={status.setup_required||register?'new-password':'current-password'} required minlength={status.setup_required||register?12:1} maxlength="256" /></label>
{#if status.setup_required||register}<label>确认密码<input type="password" bind:value={confirmation} autocomplete="new-password" required minlength="12" maxlength="256"/><small>密码至少 12 个字符。</small></label><label>账户默认时区<input list="account-timezones" bind:value={timezone} required placeholder="Asia/Shanghai"/><datalist id="account-timezones">{#each timezones as tz}<option value={tz}></option>{/each}</datalist><small>未设置独立时区的卡片会跟随此时区。</small></label>{/if}
{#if status.setup_required}<label>初始化令牌<input type="password" bind:value={token} required autocomplete="off"/><small>由部署时的 SETUP_TOKEN 提供，不能用账户密码代替。</small></label>{/if}
<Button type="submit" disabled={busy}>{busy?'正在验证…':status.setup_required?'创建管理员账户':register?'创建账户':'安全登录'}<ArrowRight size={16}/></Button>
</form>{:else}<p>正在检查服务状态…</p>{/if}
{#if status?.registration_allowed&&!status.setup_required}<button class="button button-outline mt-4" disabled={busy} onclick={()=>{register=!register;error='';confirmation='';}}>{register?'已有账户？返回登录':'创建新账户'}</button>{/if}
<div class="privacy-note mt-6"><ShieldCheck size={18}/><span>无需银行密码或完整卡号。<br/><small>本系统不会执行自动还款。</small></span></div></div>
