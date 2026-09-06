import { writable, get } from 'svelte/store';
import type { Session } from './types';
export const session = writable<Session | null>(null);
export const notice = writable<{ message: string; error: boolean } | null>(null);
export class ApiError extends Error {
  constructor(public status: number, public code: string, message: string, public requestId?: string) { super(message); }
}
export function notify(message: string, error = false) { notice.set({ message, error }); }
export function showError(error: unknown) {
  notify(error instanceof Error ? error.message : '操作失败，请刷新后重试。', true);
}
export async function api<T>(path: string, method = 'GET', body?: unknown): Promise<T> {
  const headers = new Headers({ Accept: 'application/json' });
  if (body !== undefined) headers.set('Content-Type', 'application/json');
  const csrf = get(session)?.csrf_token;
  if (method !== 'GET' && csrf) headers.set('X-CSRF-Token', csrf);
  let response: Response;
  try {
    response = await fetch(`/api/v1${path}`, {
      method, headers, credentials: 'same-origin', cache: 'no-store',
      body: body === undefined ? undefined : JSON.stringify(body), signal: AbortSignal.timeout(65000)
    });
  } catch { throw new ApiError(0, 'NETWORK_ERROR', '网络中断或请求超时。操作结果可能尚未确认，请刷新后检查，不要重复点击。'); }
  const data = await response.json().catch(() => ({}));
  if (!response.ok) {
    if (response.status === 401 && !path.startsWith('/auth/')) session.set(null);
    throw new ApiError(response.status, data.error?.code || 'HTTP_ERROR', data.error?.message || `请求失败 (${response.status})`, data.error?.request_id);
  }
  return data as T;
}
export async function refreshSession() { const result = await api<Session>('/auth/session'); session.set(result); return result; }
export async function copy(value: string) {
  try { await navigator.clipboard.writeText(value); notify('已复制。'); }
  catch { notify('无法访问剪贴板，请手动选择并复制。', true); }
}
