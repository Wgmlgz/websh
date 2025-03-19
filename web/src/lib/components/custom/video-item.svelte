<script lang="ts">
  import type { ConnectionManager } from '$lib/connection';
  import type { VideoState } from '$lib/state';
  import { onMount } from 'svelte';

  export let manager: ConnectionManager;
  export let video_state: VideoState;
  let video: HTMLDivElement;

  onMount(() => {
    manager?.startVideo(video, video_state);
  });
</script>

<div bind:this={video} />
