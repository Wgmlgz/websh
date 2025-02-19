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
  let targetServer: string = 'server1';
  let targetSession: string = '1';
  let video: HTMLVideoElement;
  let status = writable();
  let manager: ConnectionManager;
  const term = new Terminal();

  const server_url = writable('ws://localhost:8002');
  onMount(() => {
    manager = new ConnectionManager(server_url, video);
    status = manager.status;
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
  <div bind:this={video} />
</div>
