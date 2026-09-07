<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { Trophy, LayoutDashboard, CreditCard, CalendarDays, Bell, Settings, LogOut, ShieldCheck, X } from '@lucide/svelte';
  import { api, session, notice, refreshSession, showError } from '$lib/api';
  import Loading from '$lib/components/Loading.svelte';
  let { children } = $props();
  let bootError = $state('');
  const navigation = [
    { href: '/', label: '总览', icon: LayoutDashboard }, { href: '/cards', label: '信用卡', icon: CreditCard },
    { href: '/calendar', label: '日历订阅', icon: CalendarDays }, { href: '/notifications', label: '通知中心', icon: Bell },
    { href: '/ranking', label: '免息排行', icon: Trophy },
    { href: '/settings', label: '设置', icon: Settings }
  ];
  onMount(() => {
    refreshSession().then(() => { if (page.url.pathname === '/login') void goto('/'); }).catch((e) => {
      if (e.status === 401) { if (page.url.pathname !== '/login') void goto('/login'); }
      else bootError = e.message;
    });
    const media = matchMedia('(prefers-color-scheme: dark)');
    const change = () => { if ((localStorage.getItem('carddue-theme') || 'system') === 'system') document.documentElement.dataset.theme = media.matches ? 'dark' : 'light'; };
    media.addEventListener('change', change); return () => media.removeEventListener('change', change);
  });
  async function logout() { try { await api('/auth/logout', 'POST'); session.set(null); await goto('/login'); } catch (e) { showError(e); } }
</script>
<svelte:head><title>CardDue · 信用卡日程</title><meta name="description" content="私有信用卡日期、动态日历订阅与多渠道提醒。" /></svelte:head>
<a href="#content" class="skip-link">跳到主要内容</a>
{#if page.url.pathname === '/login'}
  <main id="content" class="auth-container">{@render children()}</main>
{:else if $session}
  <div class="app-shell">
    <aside class="sidebar">
      <a class="brand" href="/"><span class="brand-icon"><CreditCard size={21} /></span><span>CardDue<small>把每一个日期，放在心上</small></span></a>
      <div class="nav-label">工作空间</div>
      <nav aria-label="主导航">{#each navigation as item}
        <a href={item.href} class:active={item.href === '/' ? page.url.pathname === '/' : page.url.pathname.startsWith(item.href)} aria-current={(item.href === '/' ? page.url.pathname === '/' : page.url.pathname.startsWith(item.href)) ? 'page' : undefined}><item.icon size={19} /><span>{item.label}</span></a>
      {/each}</nav>
      <div class="sidebar-bottom"><div class="privacy-note"><ShieldCheck size={18} /><span>不连接银行<br /><small>仅保存你主动录入的信息</small></span></div><div class="account-email">{$session.user.email}</div><button class="logout" onclick={logout}><LogOut size={16} />退出登录</button></div>
    </aside>
    <div class="workspace"><header class="topbar"><span class="muted">个人财务日程 <span class="divider">/</span> <strong>CardDue</strong></span><span class="timezone">{$session.user.timezone}</span></header><main id="content" class="content">{@render children()}</main><footer>日期以银行实际账单为准 · 已还款为手工标记，不代表银行确认到账</footer></div>
  </div>
{:else}
  <main id="content" class="content">{#if bootError}<div class="alert error" role="alert">{bootError}<button class="button button-outline" onclick={() => location.reload()}>重新加载</button></div>{:else}<Loading /><a href="/login">返回登录</a>{/if}</main>
{/if}
{#if $notice}<div class:error={$notice.error} class="toast" role={$notice.error ? 'alert' : 'status'}><span>{$notice.message}</span><button aria-label="关闭提示" onclick={() => notice.set(null)}><X size={18} /></button></div>{/if}
