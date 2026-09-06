import { describe, it, expect } from 'vitest';
import { daysBetween, dateLabel } from './utils';
describe('date-only presentation',()=>{
  it('does not apply the browser timezone to dates',()=>{
    expect(daysBetween('2028-03-01','2028-02-28')).toBe(2);
    expect(daysBetween('2026-03-09','2026-03-08')).toBe(1);
    expect(daysBetween('2026-01-01','2025-12-31')).toBe(1);
  });
  it('formats without Date timezone conversion',()=>{expect(dateLabel('2026-09-06')).toBe('2026年9月6日');});
});
