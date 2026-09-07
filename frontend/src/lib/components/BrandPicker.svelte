<script lang="ts">
  import BrandIcon from './BrandIcon.svelte';
  let {id,label,value=$bindable(''),kind='bank',choices=[],color='#2563eb',maxLength=100}:{id:string;label:string;value?:string;kind?:'bank'|'network';choices:string[];color?:string;maxLength?:number}=$props();
  let open=$state(false),active=$state(-1);
  let input: HTMLInputElement;
  const filtered=$derived(choices.filter(n=>!value||n.toLowerCase().includes(value.toLowerCase())));
  function choose(name:string){input.focus();value=name;open=false;active=-1;}
  function toggle(){const next=!open;input.focus();open=next;active=-1;}
  function keydown(e:KeyboardEvent){
    if(e.key==='Escape'){open=false;return;}
    if(e.key==='ArrowDown'||e.key==='ArrowUp'){e.preventDefault();open=true;active=filtered.length?Math.max(0,Math.min(filtered.length-1,active+(e.key==='ArrowDown'?1:-1))):-1;}
    if(e.key==='Enter'&&open){e.preventDefault();if(active>=0&&filtered[active])choose(filtered[active]);else open=false;}
  }
</script>
<div class="brand-picker" onfocusout={(e)=>{if(!e.currentTarget.contains(e.relatedTarget as Node))open=false;}}>
  <label for={id}>{label}</label>
  <div class="picker-field"><BrandIcon name={value} {kind} {color}/><input bind:this={input} {id} role="combobox" aria-autocomplete="list" aria-expanded={open} aria-controls={open?id+'-options':undefined} aria-activedescendant={open&&active>=0&&filtered[active]?id+'-option-'+active:undefined} bind:value maxlength={maxLength} autocomplete="off" placeholder="选择常见选项，也可手动输入" onfocus={()=>{open=true;active=-1;}} oninput={()=>{open=true;active=-1;}} onkeydown={keydown}/><button type="button" aria-label={(open?'收起':'展开')+label} aria-expanded={open} aria-controls={open?id+'-options':undefined} onclick={toggle}>⌄</button></div>
  {#if open}<div class="picker-options" id={id+'-options'} role="listbox" aria-label={label}>
    {#each filtered as name,i}<button type="button" role="option" tabindex="-1" id={id+'-option-'+i} aria-selected={value===name} class:highlight={i===active} onclick={()=>choose(name)}><BrandIcon {name} {kind} {color}/><span>{name}</span></button>{/each}
    
  </div>{/if}
  {#if open&&!filtered.length}<p role="status" class="text-xs mt-2">使用输入的名称即可。</p>{/if}
</div>
<style>.brand-picker{position:relative;min-width:0}.brand-picker>label{margin-bottom:7px}.picker-field{display:flex;gap:7px;align-items:center}.picker-field input{flex:1;width:0}.picker-field>button{background:var(--surface);border:1px solid var(--line);border-radius:8px;padding:8px}.picker-options{position:absolute;top:100%;left:0;right:0;z-index:20;max-height:250px;overflow:auto;border:1px solid var(--line);border-radius:10px;background:var(--surface);box-shadow:0 10px 30px #0002}.picker-options button{display:flex;gap:10px;align-items:center;width:100%;text-align:left;padding:9px 12px;border:0;background:none;color:var(--text);font-size:12px}.picker-options button:hover,.picker-options button.highlight{background:var(--accent-soft)}.picker-options p{padding:12px;font-size:12px}</style>
