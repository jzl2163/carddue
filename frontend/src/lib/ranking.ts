export type RankingEntry = {id:string;name:string;issuer:string;network:string;color:string;region:string|null;timezone:string;different_timezone:boolean;different_date:boolean;local_today:string;statement_date:string;due_date:string;days:number;longest_days:number};
export type Ranking = {account_timezone:string;as_of:string;cards:RankingEntry[]};
export function rankCards(cards: RankingEntry[], mode: 'days'|'longest_days' = 'days'): RankingEntry[] {
  return [...cards].sort((a,b)=>b[mode]-a[mode]||a.name.localeCompare(b.name)||a.id.localeCompare(b.id));
}
