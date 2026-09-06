<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import { cn } from '$lib/utils';
  let { children, variant = 'default', class: className, type = 'button', ...rest }: HTMLButtonAttributes & { variant?: 'default' | 'outline' | 'ghost' | 'destructive' } = $props();
</script>
<button {type} class={cn('button', `button-${variant}`, className)} {...rest}>{@render children?.()}</button>
