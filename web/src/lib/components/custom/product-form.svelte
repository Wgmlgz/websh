<script lang="ts">
  import { superForm } from 'sveltekit-superforms';
  import SuperDebug from 'sveltekit-superforms';
  import { Field, Control, Label, FieldErrors } from '$lib/components/ui/form';
  import { Input } from '$lib/components/ui/input';
  import * as Card from '$lib/components/ui/card';
  import { z } from 'zod';
  import { zodClient } from 'sveltekit-superforms/adapters';
  import { Button } from '$lib/components/ui/button';
  import { toast } from 'svelte-sonner';
  import { type Infer } from 'sveltekit-superforms/client';
  import { Link } from '$lib/components/ui/link';
  import {
    ProductEditSchema,
    type ProductEdit,
    UnitOfMeasure,
    OrganizationType,
    Color,
    Country,
    type OrganizationSchema,
    type PersonSchema
  } from '$lib/types';
  import BaseLabel from '$lib/components/ui/label/label.svelte';
  import moment from 'moment';
  import * as Select from '../ui/select';
  import _ from 'lodash';
  import NumberInput from '../ui/input/number-input.svelte';
  export let data: ProductEdit;
  import * as Dialog from '$lib/components/ui/dialog';
  import OrganizationForm from './organization-form.svelte';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
  import { createOrganization, createPerson, organizations, persons } from '$lib/api';
  import PersonForm from './person-form.svelte';

  export let readonly = false;

  export let onSubmit: (data: ProductEdit) => any = (data) => {
    toast.success(`You submitted ${JSON.stringify(data, null, 2)}`);
  };

  const form = superForm(data, {
    SPA: true,
    dataType: 'json',
    validators: zodClient(ProductEditSchema),
    onUpdated: ({ form: f }) => {
      if (f.valid) {
        $formData = f.data;
        onSubmit(f.data);
      } else {
        console.log(f);
        toast.error('Please fix the errors in the form.');
      }
    },
    onSubmit(input) {},

    clearOnSubmit: 'errors',
    multipleSubmits: 'prevent'
  });
  const { form: formData, enhance } = form;

  $: selectedUnitOfMeasure = $formData.unitOfMeasure
    ? {
        label: _.startCase(_.toLower($formData.unitOfMeasure)),
        value: $formData.unitOfMeasure
      }
    : undefined;

  let selectedManufacturerName: string | undefined;
  $: selectedManufacturer = $formData.manufacturer?.id
    ? {
        label:
          selectedManufacturerName ??
          (('name' in $formData.manufacturer && $formData.manufacturer.name) || undefined),
        value: $formData.manufacturer?.id
      }
    : undefined;

  let selectedOwnerName: string | undefined;
  $: selectedOwner = $formData.owner?.id
    ? {
        label:
          selectedOwnerName ?? (('name' in $formData.owner && $formData.owner.name) || undefined),
        value: $formData.owner?.id
      }
    : undefined;
</script>

<div class="grid h-full w-full place-items-center items-center justify-items-center">
  <form use:enhance on:submit|preventDefault|stopPropagation class="w-full space-y-6">
    <Card.Header>
      <Card.Title>
        <slot name="title">Edit Product</slot>
      </Card.Title>
    </Card.Header>
    <Card.Content class="w-full">
      <div class="grid w-full grid-cols-3 gap-10">
        <!-- Product Name -->
        <div>
          <Field {form} name="name">
            <Control let:attrs>
              <Label>Name</Label>
              <Input {...attrs} {readonly} bind:value={$formData.name} />
            </Control>
            <FieldErrors />
          </Field>

          <!-- Coordinates -->
          <BaseLabel>Coordinates</BaseLabel>
          <div class="gap-10px grid grid-cols-2">
            <Field {form} name="coordinates.x">
              <Control let:attrs>
                <Label>x</Label>
                <NumberInput {readonly} {...attrs} bind:value={$formData.coordinates.x} />
              </Control>
              <FieldErrors />
            </Field>
            <Field {form} name="coordinates.y">
              <Control let:attrs>
                <Label>y</Label>
                <NumberInput {readonly} {...attrs} bind:value={$formData.coordinates.y} />
              </Control>
              <FieldErrors />
            </Field>
          </div>

          <!-- Price -->
          <Field {form} name="price">
            <Control let:attrs>
              <Label>Price</Label>
              <NumberInput {readonly} {...attrs} bind:value={$formData.price} />
            </Control>
            <FieldErrors />
          </Field>

          <!-- Manufacture Cost -->
          <Field {form} name="manufactureCost">
            <Control let:attrs>
              <Label>Manufacture Cost</Label>
              <NumberInput {readonly} {...attrs} bind:value={$formData.manufactureCost} />
            </Control>
            <FieldErrors />
          </Field>

          <!-- Rating -->
          <Field {form} name="rating">
            <Control let:attrs>
              <Label>Rating</Label>
              <NumberInput {readonly} {...attrs} bind:value={$formData.rating} />
            </Control>
            <FieldErrors />
          </Field>

          <!-- Unit of Measure -->
          <Field {form} name="unitOfMeasure">
            <Control let:attrs>
              <Label>Unit of Measure</Label>
              <Select.Root
                portal={null}
                selected={selectedUnitOfMeasure}
                onSelectedChange={(v) => {
                  v && ($formData.unitOfMeasure = v.value);
                }}
              >
                <Select.Trigger>
                  <Select.Value placeholder="Select Unit of Measure" />
                </Select.Trigger>
                {#if !readonly}
                  <Select.Content>
                    <Select.Group>
                      <Select.Label>Units</Select.Label>
                      {#each UnitOfMeasure.options as unit}
                        {@const label = _.startCase(_.toLower(unit))}
                        <Select.Item value={unit} {label}>
                          {label}
                        </Select.Item>
                      {/each}
                    </Select.Group>
                  </Select.Content>
                {/if}
              </Select.Root>
            </Control>
            <FieldErrors />
          </Field>
        </div>

        <div>
          {#if $formData.manufacturer != null}
            <Field {form} name="manufacturer.id">
              <Control let:attrs>
                <Label>Manufacturer</Label>

                <Select.Root
                  portal={null}
                  selected={selectedManufacturer}
                  onSelectedChange={(v) => {
                    v &&
                      $formData.manufacturer &&
                      ($formData.manufacturer.id = v.value) &&
                      (selectedManufacturerName = v.label);
                  }}
                >
                  <Select.Trigger>
                    <Select.Value placeholder="Select Manufacturer" />
                  </Select.Trigger>
                  {#if !readonly}
                    <Select.Content>
                      <Select.Group>
                        <Select.Label>Manufacturers</Select.Label>

                        {#each $organizations as organization}
                          {@const label = _.startCase(_.toLower(organization.name))}
                          <Select.Item value={organization.id} {label}>
                            {label}
                          </Select.Item>
                        {/each}
                        <Dialog.Root>
                          <Dialog.Trigger class="ml-auto">
                            <Button>Create new Organization</Button>
                          </Dialog.Trigger>
                          <Dialog.Content class="w-full max-w-[1500px]">
                            <OrganizationForm
                              onSubmit={async (data) => {
                                console.log(data);
                                await createOrganization(data);
                                toast.info('Organization created');
                              }}
                              data={{
                                name: 'Acme Corporation',
                                officialAddress: {
                                  zipCode: '',
                                  town: {
                                    x: 0,
                                    y: 0,
                                    z: 0
                                  }
                                },
                                annualTurnover: 0, // Value > 0
                                employeesCount: 0, // Value > 0
                                rating: 0, // Value > 0
                                type: 'COMMERCIAL'
                              }}
                            >
                              <svelte:fragment slot="title">Create new Organization</svelte:fragment
                              >
                              <svelte:fragment slot="button">Create</svelte:fragment>
                            </OrganizationForm>
                          </Dialog.Content>
                        </Dialog.Root>
                      </Select.Group>
                    </Select.Content>
                  {/if}
                </Select.Root>
              </Control>
              <FieldErrors />
            </Field>
            {#if !readonly}
              <Button on:click={() => ($formData.manufacturer = undefined)}>
                Remove Manufacturer
              </Button>
            {/if}
          {:else if !readonly}
            <Button on:click={() => ($formData.manufacturer = { id: 0 })}>
              Include Manufacturer
            </Button>
          {/if}
        </div>

        <div>
          <Field {form} name="owner.id">
            <Control let:attrs>
              <Label>Owner</Label>

              <Select.Root
                portal={null}
                selected={selectedOwner}
                onSelectedChange={(v) => {
                  v &&
                    $formData.owner &&
                    ($formData.owner.id = v.value) &&
                    (selectedOwnerName = v.label);
                }}
              >
                <Select.Trigger>
                  <Select.Value placeholder="Select Owner" />
                </Select.Trigger>
                {#if !readonly}
                  <Select.Content>
                    <Select.Group>
                      <Select.Label>Owners</Select.Label>

                      {#each $persons as person}
                        {@const label = _.startCase(_.toLower(person.name))}
                        <Select.Item value={person.id} {label}>
                          {label}
                        </Select.Item>
                      {/each}
                      <Dialog.Root>
                        <Dialog.Trigger class="ml-auto">
                          <Button>Create new Person</Button>
                        </Dialog.Trigger>
                        <Dialog.Content class="w-full max-w-[1500px]">
                          <PersonForm
                            onSubmit={async (data) => {
                              console.log(data);
                              await createPerson(data);
                              toast.info('Person created');
                            }}
                            data={{
                              name: '',
                              eyeColor: 'BLUE',
                              // @ts-ignore
                              birthday: '',
                              hairColor: 'GREEN',
                              nationality: 'USA',
                              location: {
                                x: 0,
                                y: 0,
                                z: 0
                              }
                            }}
                          >
                            <svelte:fragment slot="title">Create new Person</svelte:fragment>
                            <svelte:fragment slot="button">Create</svelte:fragment>
                          </PersonForm>
                        </Dialog.Content>
                      </Dialog.Root>
                    </Select.Group>
                  </Select.Content>
                {/if}
              </Select.Root>
            </Control>
            <FieldErrors />
          </Field>
          <!-- <OrganizationForm /> -->
        </div>
      </div>
    </Card.Content>
    <slot name="footer">
      <Card.Footer>
        <div class="flex w-full">
          <Button class="ml-auto" type="submit">
            <slot name="button">Update</slot>
          </Button>
        </div>
      </Card.Footer>
    </slot>
  </form>
</div>
<!-- <SuperDebug data={$formData} /> -->
