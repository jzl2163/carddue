import { test, expect } from '@playwright/test';
const ranked = [
  {id:'a',name:'日常卡',issuer:'自定义银行',color:'#2563eb',days:12},
  {id:'b',name:'旅行卡',issuer:'招商银行',color:'#2563eb',days:40},
  {id:'c',name:'备用卡',issuer:'中国银行',color:'#2563eb',days:25},
  {id:'d',name:'购物卡',issuer:'Chase',color:'#2563eb',days:30},
].map(c=>({...c,network:'Visa',longest_days:55,region:null,timezone:'Asia/Shanghai',different_timezone:false,different_date:false,local_today:'2026-09-08',statement_date:'2026-09-20',due_date:'2026-10-10'}));
test.beforeEach(async ({page})=>{
  await page.route('**/api/v1/**', async route=>{
    const path=new URL(route.request().url()).pathname;
    const body=path.endsWith('/auth/session')?{user:{email:'test@example.invalid',timezone:'Asia/Shanghai'},csrf_token:'test'}:
      path.endsWith('/dashboard')?{today:'2026-09-08',timezone:'Asia/Shanghai',events:[]}:
      path.endsWith('/cards/ranking')?{account_timezone:'Asia/Shanghai',as_of:'2026-09-08T00:00:00Z',cards:ranked}:[];
    await route.fulfill({json:body});
  });
});
for(const width of [1280,390]){
  test('custom names, quick fill and menu alignment at '+width+'px',async ({page})=>{
    await page.setViewportSize({width,height:900});
    await page.goto('/cards/new');
    const issuer=page.getByRole('combobox',{name:'发卡行',exact:true});
    await issuer.fill('我的本地银行');
    await expect(issuer).toHaveAttribute('aria-expanded','false');
    await page.getByRole('button',{name:'展开发卡行快捷选项',exact:true}).click();
    const field=page.locator('.picker-field').first();
    const menu=page.locator('.picker-options').first();
    const f=await field.boundingBox(),m=await menu.boundingBox();
    expect(f).not.toBeNull();expect(m).not.toBeNull();
    expect(Math.abs(f!.x-m!.x)).toBeLessThan(1);
    expect(Math.abs(f!.width-m!.width)).toBeLessThan(1);
    expect(Math.abs(m!.y-f!.y-f!.height-4)).toBeLessThan(1);
    await page.getByRole('option',{name:'招商银行',exact:true}).click();
    await expect(issuer).toHaveValue('招商银行');
    await expect(issuer).toHaveAttribute('aria-expanded','false');
    await page.getByRole('button',{name:'展开发卡行快捷选项',exact:true}).click();
    await issuer.fill('不存在于目录的银行');
    await expect(page.getByRole('status')).toContainText('自定义名称');
    await issuer.press('Enter');
    await expect(issuer).toHaveValue('不存在于目录的银行');
    await page.getByRole('combobox',{name:'卡组织',exact:true}).fill('自定义卡组织');
    await page.getByLabel('卡片名称',{exact:true}).fill('自定义测试卡');
    let saved:any;
    await page.route('**/api/v1/cards',async route=>{
      if(route.request().method()==='POST'){saved=route.request().postDataJSON();await route.fulfill({status:422,json:{error:{message:'测试完成，阻止跳转'}}});}
      else await route.fulfill({json:[]});
    });
    await page.getByRole('button',{name:'保存信用卡',exact:true}).click();
    await expect.poll(()=>saved?.issuer).toBe('不存在于目录的银行');
    expect(saved.network).toBe('自定义卡组织');
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
  });
}
test('overview top three and independent error recovery',async ({page})=>{
  await page.goto('/');
  const ranking=page.getByRole('region',{name:'免息排行榜',exact:true});
  await expect(ranking.locator('h3')).toHaveText(['旅行卡','购物卡','备用卡']);
  await expect(ranking.getByRole('link',{name:'查看全部 →'})).toHaveAttribute('href','/ranking');
  await page.route('**/api/v1/cards/ranking',route=>route.fulfill({status:500,json:{error:{message:'排行榜暂时不可用'}}}));
  await ranking.getByRole('button',{name:'刷新',exact:true}).click();
  await expect(ranking.getByRole('alert')).toContainText('排行榜暂时不可用');
  await expect(page.getByRole('heading',{name:'我的信用卡',exact:true})).toBeVisible();
  await page.route('**/api/v1/cards/ranking',route=>route.fulfill({json:{account_timezone:'Asia/Shanghai',as_of:'2026-09-08T00:00:00Z',cards:[]}}));
  await ranking.getByRole('button',{name:'重试',exact:true}).click();
  await expect(ranking).toContainText('添加信用卡后');
});
