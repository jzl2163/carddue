<script lang="ts">
  import BrandIcon from './BrandIcon.svelte';
  let {id,label,value=$bindable(''),kind='bank',choices=[],color='#2563eb',maxLength=100}:{id:string;label:string;value?:string;kind?:'bank'|'network';choices:string[];color?:string;maxLength?:number}=$props();
  let open=$state(false),active=$state(-1),query=$state('');
  let input: HTMLInputElement;
  const filtered=$derived(choices.filter(n=>!query||n.toLowerCase().includes(query.toLowerCase())));
  function choose(name:string){input.focus();value=name;open=false;active=-1;}
  function toggle(){const next=!open;input.focus();query='';open=next;active=-1;}
  function keydown(e:KeyboardEvent){
    if(e.key==='Escape'){open=false;active=-1;return;}
    if(e.key==='ArrowDown'||e.key==='ArrowUp'){
      e.preventDefault();
      if(!open){query='';open=true;active=e.key==='ArrowDown'?0:choices.length-1;}
      else active=filtered.length?Math.max(0,Math.min(filtered.length-1,active+(e.key==='ArrowDown'?1:-1))):-1;
    }
    if(e.key==='Enter'&&open){e.preventDefault();if(active>=0&&filtered[active])choose(filtered[active]);else open=false;}
  }
</script>
<div class="brand-picker" onfocusout={(e)=>{if(!e.currentTarget.contains(e.relatedTarget as Node)){open=false;active=-1;}}}>
  <label for={id}>{label}</label>
  <div class="picker-anchor">
    <div class="picker-field">
      <BrandIcon name={value} {kind} {color}/>
      <input bind:this={input} {id} role="combobox" aria-autocomplete="list" aria-expanded={open} aria-controls={open?id+'-options':undefined} aria-describedby={id+'-hint'} aria-activedescendant={open&&active>=0&&filtered[active]?id+'-option-'+active:undefined} bind:value maxlength={maxLength} autocomplete="off" placeholder={kind==='bank'?'输入自定义银行名称':'输入卡组织名称'} oninput={()=>{query=value;active=-1;}} onkeydown={keydown}/>
      <button type="button" aria-label={(open?'收起':'展开')+label+'快捷选项'} aria-expanded={open} aria-controls={open?id+'-options':undefined} onclick={toggle}>⌄</button>
    </div>
    {#if open}
      <div class="picker-options">
        <div id={id+'-options'} role="listbox" aria-label={label+'快捷填充'}>
          {#each filtered as name,i}<button type="button" role="option" tabindex="-1" id={id+'-option-'+i} aria-selected={value===name} class:highlight={i===active} onclick={()=>choose(name)}><BrandIcon {name} {kind} {color}/><span>{name}</span></button>{/each}
        </div>
        {#if !filtered.length}<p role="status">无匹配选项，将使用输入的自定义名称。</p>{/if}
      </div>
    {/if}
  </div>
  <small id={id+'-hint'}>可直接填写任意名称，点击右侧箭头快速填充。</small>
</div>
<style>
  .brand-picker{min-width:0;align-self:start}
  .brand-picker>label{margin-bottom:7px}
  .brand-picker>small{display:block;margin-top:7px;font-size:10px;color:var(--muted)}
  .picker-anchor{position:relative}
  .picker-field{display:flex;gap:7px;align-items:center;min-width:0;border:1px solid var(--line);border-radius:8px;padding-left:8px;background:var(--surface)}
  .picker-field input{flex:1;width:0;border:0;background:transparent;padding:10px 4px}
  .picker-field>button{align-self:stretch;flex-shrink:0;background:transparent;color:var(--muted);border:0;border-left:1px solid var(--line);border-radius:0 8px 8px 0;padding:0 12px}
  .picker-options{position:absolute;top:calc(100% + 4px);left:0;right:0;z-index:20;max-height:250px;overflow:auto;border:1px solid var(--line);border-radius:10px;background:var(--surface);box-shadow:0 10px 30px #0002}
  .picker-options button{display:flex;gap:10px;align-items:center;width:100%;text-align:left;padding:9px 12px;border:0;background:none;color:var(--text);font-size:12px}
  .picker-options button span{min-width:0;overflow-wrap:anywhere}
  .picker-options button:hover,.picker-options button.highlight{background:var(--accent-soft)}
  .picker-options p{padding:12px;font-size:12px}
</style>
