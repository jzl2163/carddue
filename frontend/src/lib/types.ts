import type { components } from './generated/api';
export type CardInput = components['schemas']['CardInput'];
export type Card = components['schemas']['CardView'];
export type Cycle = components['schemas']['CycleView'];
export type CyclePatch = components['schemas']['CyclePatch'];
export type MilestoneInput = components['schemas']['MilestoneInput'];
export type FeedInput = components['schemas']['FeedInput'];
export type TemplateInput = components['schemas']['TemplateInput'];
export type RuleInput = components['schemas']['RuleInput'];
export type Connection = components['schemas']['ConnectionView'];
export type User = components['schemas']['User'];
export type Resource<T> = { id: string; data: T };
export type Feed = Resource<FeedInput> & { url: string; last_accessed_at: string | null };
export interface Upcoming { id: string; card_id: string; cycle_id: string | null; kind: string; title: string; date: string; paid: boolean; card_name: string; color: string; amount: string | null }
export interface Dashboard { today: string; timezone: string; events: Upcoming[] }
export interface Session { user: User; csrf_token: string }
export interface Rendered { title: string; body: string; html: string; url: string; group: string; sound: string; level: string }
export interface Delivery { id: string; status: string; scheduled_at: string; sent_at: string | null; attempt_count: number; error: string | null; connection: string; kind: string; context: { card: { name: string }; event: { label: string; date: string } } }
