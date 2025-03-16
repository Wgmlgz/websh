<script lang="ts">
  import Button from '$lib/components/ui/button/button.svelte';
  import Input from '$lib/components/ui/input/input.svelte';
  import { Terminal } from '@xterm/xterm';
  import { onDestroy, onMount } from 'svelte';
  import ReconnectingWebSocket from 'reconnecting-websocket';
  import { toast } from 'svelte-sonner';
  import { ConnectionManager, type ConnectionData } from '$lib/connection';
  import { get, writable, type Writable } from 'svelte/store';
  import * as Card from '$lib/components/ui/card';
  import { FitAddon } from '@xterm/addon-fit';

  let terminal: HTMLDivElement;
  export let connection: ConnectionData;
  const term = new Terminal({});

  let status: Writable<string>;
  let manager: ConnectionManager | null = null;

  const onResize = (fitAddon: FitAddon) => {
    console.log('sus');
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
    // fitAddon.fit();

    const xterm_resize_ob = new ResizeObserver(function (entries) {
      // since we are observing only a single element, so we access the first element in entries array
      onResize(fitAddon);
    });

    xterm_resize_ob.observe(terminal);

    manager = new ConnectionManager(
      connection.serverUrl,
      connection.targetServer,
      connection.targetSession,
      term,
    );
    status = manager.status;
  });

  onDestroy(() => {});
  $: console.log($status);
</script>

<Card.Root>
  <Card.Header>
    <Card.Title>
      {connection.targetSession}@{connection.targetServer} via {connection.serverUrl}
    </Card.Title>
    <Card.Description>Status: {$status}</Card.Description>
  </Card.Header>
  <Card.Content>
    <div bind:this={terminal}></div>
  </Card.Content>
  <Card.Footer>
    <p>Card Footer</p>
  </Card.Footer>
</Card.Root>
