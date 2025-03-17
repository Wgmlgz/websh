<script lang="ts">
  import Init from '$lib/components/custom/init.svelte';
  import Shell from '$lib/components/custom/connection-card.svelte';
  import { state, type ConnectionData } from '$lib/state';
  
  const startConnection = (connection: ConnectionData) => {
    state.update((state) => {
      state.connections = [...state.connections, connection];
      return state;
    });
  };
</script>

<div>
  <Init sus={startConnection} />
  <div class="grid">
    {#if $state.connections.length}
      {#each $state.connections as connection (connection.id)}
        <Shell {connection} />
      {/each}
    {:else}
      <p>No active shells</p>
    {/if}
  </div>
</div>
