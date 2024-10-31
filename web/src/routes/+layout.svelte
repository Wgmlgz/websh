<script>
  import '../app.pcss';
  import '@xterm/xterm/css/xterm.css'
  // import 'virtual:uno.css';
  import Sun from 'svelte-radix/Sun.svelte';
  import Moon from 'svelte-radix/Moon.svelte';
  import { toggleMode } from 'mode-watcher';
  import Button from '$lib/components/ui/button/button.svelte';
  import { ModeWatcher } from 'mode-watcher';
  import Header from '$lib/components/site/header.svelte';
  import { onMount } from 'svelte';
  import { Toaster, toast } from 'svelte-sonner';
</script>

<Toaster />

<ModeWatcher />

<Header />

<div class="w-100vw h-[calc(100vh-70px)] p-10">
  <slot></slot>
</div>
