<script lang="ts">
  import Button from '$lib/components/ui/button/button.svelte';
  import Input from '$lib/components/ui/input/input.svelte';
  import { Terminal } from '@xterm/xterm';
  import { onDestroy, onMount } from 'svelte';
  import ReconnectingWebSocket from 'reconnecting-websocket';
  import { toast } from 'svelte-sonner';
  import { ConnectionManager } from '$lib/connection';
  import { get, writable, type Writable } from 'svelte/store';
  import * as Card from '$lib/components/ui/card';
  import { v4 as uuidv4 } from 'uuid';
  import { FitAddon } from '@xterm/addon-fit';
  import type { ConnectionData } from '$lib/state';

  let terminal: HTMLDivElement;
  export let connection: ConnectionData;
  const term = new Terminal({});

  let status: Writable<string>;
  let manager: ConnectionManager | null = null;

  let video: HTMLDivElement;
  const onResize = (fitAddon: FitAddon) => {
    try {
      fitAddon.fit();
    } catch (err) {
      console.log(err);
    }
  };
  onMount(() => {
    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(terminal);

    const xterm_resize_ob = new ResizeObserver(function (entries) {
      onResize(fitAddon);
    });

    xterm_resize_ob.observe(terminal);

    manager = new ConnectionManager(connection.serverUrl, connection.targetServer);
    status = manager.status;
  });

  onDestroy(() => {});
  $: console.log($status);
</script>

<Card.Root>
  <Card.Header>
    <Card.Title>
      {connection.targetServer}@{connection.serverUrl}
    </Card.Title>
    <Card.Description>Status: {$status}</Card.Description>
  </Card.Header>
  <Card.Content>
    <div bind:this={terminal}></div>
    <Button on:click={() => manager?.startWebShell(term, uuidv4())}>Start terminal</Button>
    <div bind:this={video} />
    <Button on:click={() => manager?.startVideo(video, 0)}>Start Video</Button>
  </Card.Content>
  <Card.Footer>
    <p>Card Footer</p>
  </Card.Footer>
</Card.Root>
