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
  import { state, type ConnectionData, type VideoState } from '$lib/state';
  import VideoItem from './video-item.svelte';

  export let connection: ConnectionData;
  const term = new Terminal({});

  let terminal: HTMLDivElement;

  let status: Writable<string>;
  let manager: ConnectionManager | null = null;

  $: ready = manager?.ready;
  $: if (terminal) initTerminal(terminal);

  const initTerminal = (terminal: HTMLDivElement) => {
    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(terminal);

    const xterm_resize_ob = new ResizeObserver(function (entries) {
      onResize(fitAddon);
    });

    xterm_resize_ob.observe(terminal);
  };

  const onResize = (fitAddon: FitAddon) => {
    try {
      fitAddon.fit();
    } catch (err) {
      console.log(err);
    }
  };

  onMount(() => {
    manager = new ConnectionManager(connection.serverUrl, connection.targetServer);
    status = manager.status;
  });

  const addVideo = (video_state: VideoState) => {
    state.update((state) => {
      connection.videos ??= [];
      connection.videos.push(video_state);
      return state;
    });
    connection.videos;
  };
  onDestroy(() => {});
  $: console.log($status);
</script>

{#if manager != null}
  <Card.Root>
    <Card.Header>
      <Card.Title>
        {connection.targetServer}@{connection.serverUrl}
      </Card.Title>
      <Card.Description>Status: {$status}</Card.Description>
    </Card.Header>
    <Card.Content>
      {#if $ready}
        <!-- content here -->
        <div bind:this={terminal}></div>
        <Button on:click={() => manager?.startWebShell(term, uuidv4())}>Start terminal</Button>
        {#each connection.videos ?? [] as video_state (video_state.id)}
          <VideoItem bind:manager {video_state} />
        {/each}
        <Button
          on:click={() =>
            addVideo({
              id: uuidv4(),
              display_id: (connection.videos ?? []).length + 1,
              width: 1920,
              height: 1080,
              refresh_rate: 60
            })}
        >
          Start Video
        </Button>
      {/if}
    </Card.Content>
    <Card.Footer>
      <p>Card Footer</p>
    </Card.Footer>
  </Card.Root>
{/if}
