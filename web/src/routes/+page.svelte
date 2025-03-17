<script lang="ts">
  import Init from '$lib/components/custom/init.svelte';
  import Shell from '$lib/components/custom/shell.svelte';
  import type { ConnectionData } from '$lib/connection';

  let connections: ConnectionData[] = [];
  const startConnection = (connection: ConnectionData) => {
    connections = [...connections, connection];
  };
</script>

<div>
  <Init sus={startConnection} />
  <div class="grid">
    {#if connections.length}
      {#each connections as connection}
        <Shell {connection} />
      {/each}
    {:else}
      <p>No active shells</p>
    {/if}
  </div>
</div>
