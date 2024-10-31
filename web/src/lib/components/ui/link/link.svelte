<script lang="ts">
  import type { HTMLAnchorAttributes } from 'svelte/elements';
  import { cn } from '$lib/utils.js';

  type $$Props = HTMLAnchorAttributes & { error?: string };

  let className: $$Props['class'] = undefined;
  let href: $$Props['href'] = undefined;
  export { className as class };
</script>

<a
  class={cn('text-primary font-medium underline underline-offset-4', className)}
  {href}
  {...$$restProps}
>
  <slot></slot>
</a>
