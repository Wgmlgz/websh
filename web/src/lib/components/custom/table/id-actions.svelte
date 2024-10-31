<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import * as Dialog from '$lib/components/ui/dialog';
  import ProductsForm from '../product-form.svelte';
  import type { Product } from '$lib/types';
  export let data: Product;

  export let id: string;
</script>

<div class="gap-10px flex items-center justify-items-center">
  <Dialog.Root>
    <Dialog.Trigger let:builder>
      <Button builders={[builder]} variant="link">
        {id}
      </Button>
    </Dialog.Trigger>
    <Dialog.Content class="w-full max-w-[1500px]">
      <!-- <Dialog.Header>
    <Dialog.Title>Are you sure absolutely sure?</Dialog.Title>
    <Dialog.Description>
      This action cannot be undone. This will permanently delete your account and remove your data
      from our servers.
    </Dialog.Description>
  </Dialog.Header> -->

      <ProductsForm {data} readonly>
        <div slot="title">Product Info</div>
        <div slot="footer"></div>
      </ProductsForm>
    </Dialog.Content>
  </Dialog.Root>
</div>
