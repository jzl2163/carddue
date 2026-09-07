export const regions = [
  ['CN','中国大陆'],['HK','中国香港'],['MO','中国澳门'],['TW','中国台湾'],['US','美国'],['CA','加拿大'],
  ['GB','英国'],['AU','澳大利亚'],['NZ','新西兰'],['JP','日本'],['KR','韩国'],['SG','新加坡'],
  ['MY','马来西亚'],['TH','泰国'],['DE','德国'],['FR','法国'],['CH','瑞士'],['IN','印度'],['AE','阿联酋']
] as const;
export const timezones = ['Asia/Shanghai','Asia/Hong_Kong','Asia/Macau','Asia/Taipei','Asia/Tokyo','Asia/Seoul','Asia/Singapore','Asia/Kuala_Lumpur','Asia/Bangkok','Asia/Kolkata','Asia/Dubai','America/New_York','America/Chicago','America/Denver','America/Los_Angeles','America/Phoenix','Pacific/Honolulu','America/Toronto','America/Vancouver','Europe/London','Europe/Berlin','Europe/Paris','Europe/Zurich','Australia/Sydney','Australia/Perth','Pacific/Auckland','UTC'];
export const networks = ['Visa','Mastercard','银联','American Express','JCB','Discover','Diners Club'];
export const banks = [
  {name:'中国工商银行',regions:['CN']},{name:'中国建设银行',regions:['CN']},{name:'中国银行',regions:['CN','HK','MO']},
  {name:'中国农业银行',regions:['CN']},{name:'交通银行',regions:['CN','HK']},{name:'招商银行',regions:['CN']},
  {name:'中信银行',regions:['CN']},{name:'中国光大银行',regions:['CN']},{name:'民生银行',regions:['CN']},
  {name:'兴业银行',regions:['CN']},{name:'浦发银行',regions:['CN']},{name:'平安银行',regions:['CN']},
  {name:'广发银行',regions:['CN']},{name:'中国邮政储蓄银行',regions:['CN']},{name:'北京银行',regions:['CN']},
  {name:'HSBC 汇丰银行',regions:['HK','GB','US','SG']},{name:'恒生银行',regions:['HK']},
  {name:'渣打银行',regions:['HK','SG','GB']},{name:'DBS 星展银行',regions:['SG','HK','TW']},
  {name:'OCBC 华侨银行',regions:['SG','MY']},{name:'UOB 大华银行',regions:['SG','MY','TH']},
  {name:'Chase',regions:['US']},{name:'Citi 花旗银行',regions:['US','HK','SG']},
  {name:'Bank of America',regions:['US']},{name:'Capital One',regions:['US']},
  {name:'American Express',regions:['US','CA','GB','AU']},{name:'Wells Fargo',regions:['US']},
  {name:'RBC',regions:['CA']},{name:'TD',regions:['CA','US']},{name:'CIBC',regions:['CA']},
  {name:'Barclays',regions:['GB']},{name:'三井住友银行',regions:['JP']},{name:'楽天カード',regions:['JP']},
  {name:'国泰世华银行',regions:['TW']},{name:'玉山银行',regions:['TW']},{name:'台新银行',regions:['TW']},
  {name:'Commonwealth Bank',regions:['AU']},{name:'ANZ',regions:['AU','NZ']}
];
export function regionLabel(code?: string | null) { return regions.find(r=>r[0]===code)?.[1] || code || '未指定地区'; }
