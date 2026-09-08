import { describe, expect, it } from 'vitest';
import { rankCards, type RankingEntry } from './ranking';
const entry = (id: string, name: string, days: number, longest_days: number): RankingEntry => ({
  id, name, days, longest_days, issuer: '', network: '', color: '#2563eb', region: null,
  timezone: 'Asia/Shanghai', different_timezone: false, different_date: false,
  local_today: '2026-09-08', statement_date: '2026-09-20', due_date: '2026-10-10'
});
describe('interest-free ranking shared by overview and full list', () => {
  it('ranks today independently of theoretical longest without mutating API data', () => {
    const cards = [entry('a','A',12,55),entry('b','B',40,45),entry('c','C',25,60),entry('d','D',30,50)];
    expect(rankCards(cards).slice(0,3).map(c=>c.id)).toEqual(['b','d','c']);
    expect(rankCards(cards,'longest_days').map(c=>c.id)).toEqual(['c','a','d','b']);
    expect(cards.map(c=>c.id)).toEqual(['a','b','c','d']);
  });
  it('breaks ties consistently by name then ID', () => {
    expect(rankCards([entry('2','B',20,30),entry('3','A',20,30),entry('1','A',20,30)]).map(c=>c.id)).toEqual(['1','3','2']);
  });
  it('handles no cards and a single card', () => {
    expect(rankCards([])).toEqual([]);
    const card = entry('a','A',0,30);
    expect(rankCards([card])).toEqual([card]);
  });
});
