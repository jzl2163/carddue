<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import type { RankingData } from '$lib/types';
  import BrandIcon from '$lib/components/BrandIcon.svelte';

  let data = $state<RankingData | null>(null);
  let error = $state('');
  let loading = $state(false);

  const ranked = $derived(
    [...(data?.cards ?? [])]
      .sort((a, b) => b.days - a.days || a.name.localeCompare(b.name))
      .slice(0, 3)
  );

  async function load() {
    if (loading) return;
    loading = true;
    error = '';
    try {
      data = await api<RankingData>('/cards/ranking');
    } catch (e) {
      error = e instanceof Error ? e.message : '无法加载免息排行。';
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    void load();
  });
</script>

<section class="panel ranking-preview" aria-labelledby="ranking-preview-title">
  <div class="panel-heading">
    <div>
      <h2 id="ranking-preview-title">免息排行</h2>
      <p>今天消费，预计免息期最长的前三张卡</p>
    </div>
    <a class="subtle-link preview-link" href="/ranking">查看完整排行 →</a>
  </div>

  {#if error}
    <div class="ranking-error" role="alert">
      <span>{error}</span>
      <button class="button button-outline" type="button" onclick={load} disabled={loading}>
        {loading ? '加载中…' : '重试'}
      </button>
    </div>
  {:else if !data}
    <div class="ranking-loading" role="status" aria-live="polite">
      <span class="skeleton loading-line"></span>
      <span class="skeleton loading-line short"></span>
      <span class="muted">正在读取最新信息…</span>
    </div>
  {:else if !ranked.length}
    <div class="ranking-empty">
      <span class="empty-mark" aria-hidden="true">＋</span>
      <div>
        <h3>添加信用卡后开始比较</h3>
        <p>排行榜只包含未归档的信用卡。</p>
      </div>
    </div>
  {:else}
    <ol class="ranking-list">
      {#each ranked as card, index (card.id)}
        <li>
          <a class="ranking-item" href={'/cards/' + card.id}>
            <span class="rank-number" aria-label={'第 ' + (index + 1) + ' 名'}>{index + 1}</span>
            <BrandIcon name={card.issuer} color={card.color} />
            <span class="ranking-card-name">{card.name}</span>
            <span class="ranking-due">
              <small>预计还款日</small>
              <strong>{card.due_date}</strong>
            </span>
            <span class="ranking-days"><strong>{card.days}</strong> 天</span>
          </a>
          {#if card.different_timezone || card.different_date}
            <small class="ranking-timezone">
              {#if card.different_timezone}卡片时区 {card.timezone}{/if}
              {#if card.different_timezone && card.different_date} · {/if}
              {#if card.different_date}当地日期为 {card.local_today}{/if}
            </small>
          {/if}
        </li>
      {/each}
    </ol>
  {/if}
</section>

<style>
  .ranking-preview {
    min-width: 0;
  }

  .ranking-preview .panel-heading {
    align-items: flex-start;
  }

  .ranking-preview .panel-heading p {
    margin-top: 4px;
    font-size: 11px;
  }

  .preview-link {
    flex: none;
    padding-top: 2px;
    white-space: nowrap;
  }

  .ranking-list {
    display: grid;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .ranking-list li {
    min-width: 0;
    border-top: 1px solid var(--line);
  }

  .ranking-item {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    padding: 11px 0 8px;
  }

  .ranking-item:hover .ranking-card-name {
    color: var(--accent);
  }

  .rank-number {
    width: 18px;
    flex: none;
    color: var(--muted);
    font-size: 14px;
    font-weight: 700;
    text-align: center;
  }

  .ranking-card-name {
    min-width: 0;
    flex: 1;
    overflow: hidden;
    font-size: 13px;
    font-weight: 620;
    text-overflow: ellipsis;
    white-space: nowrap;
    transition: color 0.15s;
  }

  .ranking-due {
    display: grid;
    flex: none;
    gap: 1px;
    color: var(--muted);
    font-size: 11px;
    line-height: 1.35;
    text-align: right;
  }

  .ranking-due small {
    font-size: 9px;
  }

  .ranking-due strong {
    color: var(--text);
    font-size: 11px;
    font-weight: 550;
  }

  .ranking-days {
    min-width: 54px;
    flex: none;
    color: var(--muted);
    font-size: 11px;
    text-align: right;
    white-space: nowrap;
  }

  .ranking-days strong {
    color: var(--text);
    font-size: 21px;
    font-weight: 700;
    letter-spacing: -0.5px;
  }

  .ranking-timezone {
    display: block;
    padding: 0 0 9px 28px;
    color: var(--accent);
    font-size: 10px;
    line-height: 1.45;
  }

  .ranking-loading {
    display: grid;
    gap: 9px;
    padding: 10px 0 3px;
    font-size: 11px;
  }

  .loading-line {
    display: block;
    height: 13px;
    width: 100%;
  }

  .loading-line.short {
    width: 68%;
  }

  .ranking-empty {
    display: flex;
    align-items: center;
    gap: 13px;
    padding: 8px 0 2px;
  }

  .ranking-empty .empty-mark {
    width: 42px;
    height: 42px;
    flex: none;
    margin: 0;
    border-radius: 13px;
    font-size: 24px;
  }

  .ranking-empty h3 {
    font-size: 13px;
  }

  .ranking-empty p {
    margin-top: 3px;
    font-size: 11px;
  }

  .ranking-error {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 9px 0 1px;
    color: var(--danger);
    font-size: 11px;
  }

  .ranking-error .button {
    min-height: 33px;
    padding: 7px 11px;
    font-size: 11px;
  }

  @media (max-width: 520px) {
    .ranking-preview .panel-heading {
      gap: 8px;
    }

    .preview-link {
      font-size: 11px;
    }

    .ranking-item {
      gap: 7px;
    }

    .ranking-due small {
      display: none;
    }

    .ranking-due strong {
      font-size: 10px;
    }

    .ranking-days {
      min-width: 46px;
    }

    .ranking-days strong {
      font-size: 19px;
    }
  }

  @media (max-width: 380px) {
    .ranking-preview .panel-heading {
      align-items: flex-start;
      flex-wrap: wrap;
    }

    .preview-link {
      margin-left: 31px;
    }

    .ranking-due {
      display: none;
    }
  }
</style>
