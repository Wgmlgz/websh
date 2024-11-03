<script lang="ts">
  import Button from '$lib/components/ui/button/button.svelte';
  import Input from '$lib/components/ui/input/input.svelte';
  import { Terminal } from '@xterm/xterm';
  import { onMount } from 'svelte';
  import ReconnectingWebSocket from 'reconnecting-websocket';
  import { toast } from 'svelte-sonner';
  import { ConnectionManager } from '$lib/connection';
  import { get, writable } from 'svelte/store';

  let terminal: HTMLDivElement;
  let targetServer: string;
  let targetSession: string;

  const term = new Terminal();

  const server_url = writable('wss://dev-websh.amogos.pro/signaling');
  let manager = new ConnectionManager(server_url);
  let status = manager.status;
  onMount(() => {
    term.open(terminal);
  });

  const startSession = () => {
    manager.startSession(targetServer, targetSession, term);
  };
  $: console.log($status);
</script>

<div>
  Server url:
  <Input bind:value={$server_url} />

  <p>Status: {$status}</p>
  <div bind:this={terminal}></div>
  Target Server Name:
  <Input bind:value={targetServer} />
  Target Session Name:
  <Input bind:value={targetSession} />
  <Button on:click={startSession}>Start Session</Button>
  <div id="logs"></div>
</div>
