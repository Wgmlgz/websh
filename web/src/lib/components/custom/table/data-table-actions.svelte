<script lang="ts">
  import Ellipsis from 'lucide-svelte/icons/ellipsis';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
  import { Button } from '$lib/components/ui/button';
  import * as Dialog from '$lib/components/ui/dialog';
  import ProductsForm from '../product-form.svelte';
  import type { Product } from '$lib/types';
  import Pencil from 'svelte-radix/Pencil1.svelte';
  import Trash from 'svelte-radix/Trash.svelte';
  import { deleteProduct, updateProduct } from '$lib/api';
  import { toast } from 'svelte-sonner';
  import * as AlertDialog from '$lib/components/ui/alert-dialog/index.js';
  export let data: Product;

  export let id: string;
</script>

<div class="gap-10px flex items-center justify-items-center">
  <!-- <DropdownMenu.Root>
    <DropdownMenu.Trigger asChild let:builder>
      <Button variant="ghost" builders={[builder]} size="icon" class="relative h-8 w-8 p-0">
        <span class="sr-only">Open menu</span>
        <Ellipsis class="h-4 w-4" />
      </Button>
    </DropdownMenu.Trigger>
    <DropdownMenu.Content>
      <DropdownMenu.Group>
        <DropdownMenu.Label>Actions</DropdownMenu.Label>
        <DropdownMenu.Item on:click={() => navigator.clipboard.writeText(id)}>
          Copy payment ID
        </DropdownMenu.Item>
      </DropdownMenu.Group>
      <DropdownMenu.Separator />
      <DropdownMenu.Item></DropdownMenu.Item>
      <DropdownMenu.Item>View payment details</DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root> -->

  <AlertDialog.Root>
    <AlertDialog.Trigger asChild let:builder>
      <Button builders={[builder]} variant="ghost">
        <Trash class="h-[1.2rem] w-[1.2rem]" />
      </Button>
    </AlertDialog.Trigger>
    <AlertDialog.Content>
      <AlertDialog.Header>
        <AlertDialog.Title>Are you absolutely sure?</AlertDialog.Title>
        <AlertDialog.Description>
          This action cannot be undone.
        </AlertDialog.Description>
      </AlertDialog.Header>
      <AlertDialog.Footer>
        <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
        <AlertDialog.Action on:click={() => deleteProduct(id)}>Delete</AlertDialog.Action>
      </AlertDialog.Footer>
    </AlertDialog.Content>
  </AlertDialog.Root>

  <Dialog.Root>
    <Dialog.Trigger let:builder>
      <Button builders={[builder]} variant="ghost">
        <Pencil class="h-[1.2rem] w-[1.2rem]" />
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

      <ProductsForm
        {data}
        onSubmit={async (data) => {
          await updateProduct(data);
          toast.info('Product updated');
        }}
      />
    </Dialog.Content>
  </Dialog.Root>
</div>
