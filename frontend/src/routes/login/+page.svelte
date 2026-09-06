<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { CreditCard, ArrowRight, ShieldCheck } from '@lucide/svelte';
  import { api, session } from '$lib/api';
  import type { Session } from '$lib/types';
  import Button from '$lib/components/ui/button/Button.svelte';
  let status = $state<{setup_required:boolean;registration_allowed:boolean;default_timezone:string} | null>(null);
  let email = $state(''), password = $state(''), token = $state(''), timezone = $state('Asia/Shanghai'), error = $state(''), busy = $state(false), register = $state(false);
  onMount(() => { api<typeof status>('/auth/status').then(v => {status=v;timezone=v?.default_timezone || 'Asia/Shanghai';}).catch(e=>error=e.message); });
  async function submit(event: SubmitEvent) {
    event.preventDefault(); busy=true; error='';
    try {
      const setup=status?.setup_required;
      const result=await api<Session>(setup?'/auth/setup':register?'/auth/register':'/auth/login','POST',setup||register?{email,password,timezone,setup_token:setup?token:null}:{email,password});
      password='';token='';session.set(result);await goto('/');
    } catch(e) {error=e instanceof Error?e.message:'无法登录';} finally {busy=false;}
  }
</script>
<div class="panel auth-card"><div class="brand"><span class="brand-icon"><CreditCard size={22}/></span><span>CardDue<small>YOUR DATES, IN ONE PLACE</small></span></div>
<h1>{status?.setup_required?'建立你的私有工作空间':register?'创建账户':'欢迎回来'}</h1><p>集中管理信用卡日期，让提醒按你的方式到达。</p>
{#if error}<div class="alert error" role="alert">{error}</div>{/if}
{#if status}<form onsubmit={submit}>
<label>邮箱<input type="email" bind:value={email} autocomplete="username" required maxlength="254" /></label>
<label>密码<input type="password" bind:value={password} autocomplete={status.setup_required||register?'new-password':'current-password'} required minlength={status.setup_required||register?12:1} maxlength="256" /></label>
{#if status.setup_required||register}<label>时区<input bind:value={timezone} required placeholder="Asia/Shanghai"/><small>使用 IANA 名称，提醒时间按照该时区计算。</small></label>{/if}
{#if status.setup_required}<label>初始化令牌<input type="password" bind:value={token} required autocomplete="off"/><small>由部署时的 SETUP_TOKEN 提供，不能用账户密码代替。</small></label>{/if}
<Button type="submit" disabled={busy}>{busy?'正在验证…':status.setup_required?'创建管理员账户':register?'创建账户':'安全登录'}<ArrowRight size={16}/></Button>
</form>{:else}<p>正在检查服务状态…</p>{/if}
{#if status?.registration_allowed&&!status.setup_required}<button class="subtle-link mt-4" onclick={()=>register=!register}>{register?'已有账户？返回登录':'创建新账户'}</button>{/if}
<div class="privacy-note mt-6"><ShieldCheck size={18}/><span>无需银行密码或完整卡号。<br/><small>本系统不会执行自动还款。</small></span></div></div>
