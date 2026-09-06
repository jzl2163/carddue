import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
export function cn(...inputs: ClassValue[]) { return twMerge(clsx(inputs)); }
export function daysBetween(date: string, today: string) {
  return Math.round((Date.parse(`${date}T00:00:00Z`) - Date.parse(`${today}T00:00:00Z`)) / 86400000);
}
export function dateLabel(date: string) {
  const [year, month, day] = date.split('-'); return `${year}年${Number(month)}月${Number(day)}日`;
}
export const kindLabels: Record<string, string> = { payment_due: '还款日', statement: '账单日', annual_fee: '年费日', benefit: '权益重置', custom: '自定义事项' };
export const channelLabels: Record<string, string> = { bark: 'Bark', email: '电子邮件', telegram: 'Telegram', webpush: '浏览器推送', webhook: 'Webhook' };
export const statusLabels: Record<string, string> = { pending: '等待发送', processing: '发送中', retry: '等待重试', sent: '服务商已接收', cancelled: '已取消', dead: '发送失败', expired: '已过期' };
export function setTheme(value: string) {
  localStorage.setItem('carddue-theme', value);
  document.documentElement.dataset.theme = value === 'dark' || (value === 'system' && matchMedia('(prefers-color-scheme: dark)').matches) ? 'dark' : 'light';
}
