import type { CardInput, FeedInput, TemplateInput } from './types';
export function defaultCard(): CardInput {
  return { name: '', region: null, timezone: null, issuer: '', network: '', last4: '', color: '#2563eb', currency: 'CNY', statement_day: 5, statement_last_day: false, due_mode: 'fixed_day', due_day: 25, due_last_day: false, due_month_offset: 0, due_offset_days: 20, weekend_adjustment: 'none', annual_fee_month: null, annual_fee_day: null, annual_fee_amount: null, notes: '' };
}
export function defaultFeed(): FeedInput {
  return { name: '我的信用卡日历', enabled: true, privacy: 'normal', kinds: ['payment_due', 'statement', 'annual_fee', 'benefit', 'custom'], card_ids: [], hide_paid: false, alarms_days_before: [] };
}
export function defaultTemplate(): TemplateInput {
  return { name: '新的通知模板', title: '💳 {{ card.name }} · {{ event.label }}', body: '日期：{{ event.date }}\n距离到期：{{ event.days_until }} 天{% if cycle.amount %}\n金额：{{ cycle.amount }} {{ card.currency }}{% endif %}', html: '<h2>{{ card.name }}</h2><p>{{ event.label }}：<strong>{{ event.date }}</strong></p><p>请以银行实际账单为准。</p>', url: '{{ app.url }}/cards/{{ card.id }}', group: 'CardDue', sound: 'bell', level: 'active' };
}
