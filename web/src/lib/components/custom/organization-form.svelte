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
    OrganizationSchema,
    type Organization,
    UnitOfMeasure,
    OrganizationType,
    Color,
    Country,
    type PersonSchema
  } from '$lib/types';
  import BaseLabel from '$lib/components/ui/label/label.svelte';
  import moment from 'moment';
  import * as Select from '../ui/select';
  import _ from 'lodash';
  import NumberInput from '../ui/input/number-input.svelte';
  export let data: Organization;
  import * as Dialog from '$lib/components/ui/dialog';
  import Checkbox from '../ui/checkbox/checkbox.svelte';

  export let onSubmit: (data: Organization) => any = (data) => {
    toast.success(`You submitted ${JSON.stringify(data, null, 2)}`);
  };

  const form = superForm(data, {
    SPA: true,
    dataType: 'json',
    validators: zodClient(OrganizationSchema),
    onUpdated: ({ form: f }) => {
      if (f.valid) {
        $formData = f.data;
        onSubmit(f.data);
      } else {
        toast.error('Please fix the errors in the form.');
      }
    },
    onSubmit(input) {},

    clearOnSubmit: 'errors',
    multipleSubmits: 'prevent'
  });
  const { form: formData, enhance } = form;

  $: selectedOrganizationType = $formData?.type
    ? {
        label: _.startCase(_.toLower($formData.type)),
        value: $formData.type
      }
    : undefined;
</script>

<div class="grid h-full w-full place-items-center items-center justify-items-center">
  <form use:enhance on:submit|preventDefault|stopPropagation class="w-full space-y-6">
    <Card.Header>
      <Card.Title>
        <slot name="title">Edit Organization</slot>
      </Card.Title>
    </Card.Header>
    <Card.Content class="w-full">
      <div class="grid w-full grid-cols-2 gap-10">
        <!-- Manufacturer -->
        <div>
          <BaseLabel>Manufacturer</BaseLabel>
          <!-- Manufacturer Fields -->
          <Field {form} name="name">
            <Control let:attrs>
              <Label>Name</Label>
              <Input {...attrs} bind:value={$formData.name} />
            </Control>
            <FieldErrors />
          </Field>
          <Field {form} name="annualTurnover">
            <Control let:attrs>
              <Label>Annual turnover</Label>
              <NumberInput {...attrs} bind:value={$formData.annualTurnover} />
            </Control>
            <FieldErrors />
          </Field>
          <Field {form} name="employeesCount">
            <Control let:attrs>
              <Label>Employees Count</Label>
              <NumberInput {...attrs} bind:value={$formData.employeesCount} />
            </Control>
            <FieldErrors />
          </Field>
          <Field {form} name="rating">
            <Control let:attrs>
              <Label>Rating</Label>
              <NumberInput {...attrs} bind:value={$formData.rating} />
            </Control>
            <FieldErrors />
          </Field>
          <!-- Organization Type -->
          <Field {form} name="type">
            <Control let:attrs>
              <Label>Organization Type</Label>
              <Select.Root
                portal={null}
                selected={selectedOrganizationType}
                onSelectedChange={(v) => {
                  v && $formData && ($formData.type = v.value);
                }}
              >
                <Select.Trigger>
                  <Select.Value placeholder="Select Organization Type" />
                </Select.Trigger>
                <Select.Content>
                  <Select.Group>
                    <Select.Label>Types</Select.Label>
                    {#each OrganizationType.options as type}
                      {@const label = _.startCase(_.toLower(type))}
                      <Select.Item value={type} {label}>
                        {label}
                      </Select.Item>
                    {/each}
                  </Select.Group>
                </Select.Content>
              </Select.Root>
            </Control>
            <FieldErrors />
          </Field>
        </div>
        <div>
          <BaseLabel>Official address</BaseLabel>
          <br />
          {#if $formData.officialAddress != null}
            {#if $formData.officialAddress.town != null}
              <BaseLabel>Town</BaseLabel>
              <div class="gap-10px grid grid-cols-3">
                <Field {form} name="officialAddress.town.x">
                  <Control let:attrs>
                    <Label>x</Label>
                    {#if $formData?.officialAddress?.town?.x !== undefined}
                      <NumberInput {...attrs} bind:value={$formData.officialAddress.town.x} />
                    {/if}
                  </Control>
                  <FieldErrors />
                </Field>
                <Field {form} name="officialAddress.town.y">
                  <Control let:attrs>
                    <Label>y</Label>
                    {#if $formData?.officialAddress?.town?.y !== undefined}
                      <NumberInput {...attrs} bind:value={$formData.officialAddress.town.y} />
                    {/if}
                  </Control>
                  <FieldErrors />
                </Field>
                <Field {form} name="officialAddress.town.z">
                  <Control let:attrs>
                    <Label>z</Label>
                    {#if $formData?.officialAddress?.town?.z !== undefined}
                      <NumberInput {...attrs} bind:value={$formData.officialAddress.town.z} />
                    {/if}
                  </Control>
                  <FieldErrors />
                </Field>
              </div>

              <Button
                on:click={() =>
                  $formData.officialAddress && ($formData.officialAddress.town = undefined)}
              >
                Remove town
              </Button>
            {:else}
              <Button
                on:click={() =>
                  $formData.officialAddress &&
                  ($formData.officialAddress.town = { x: 0, y: 0, z: 0 })}
              >
                Include town
              </Button>
            {/if}
            <Field {form} name="officialAddress.zipCode">
              <Control let:attrs>
                <Label>Zip Code</Label>
                <Input {...attrs} bind:value={$formData.officialAddress.zipCode} />
              </Control>
              <FieldErrors />
            </Field>
            <Button on:click={() => ($formData.officialAddress = undefined)}>Remove address</Button>
          {:else}
            <Button
              on:click={() =>
                ($formData.officialAddress = { town: { x: 0, y: 0, z: 0 }, zipCode: '' })}
            >
              Include Official address
            </Button>
          {/if}
        </div>
      </div>
    </Card.Content>
    <Card.Footer>
      <div class="flex w-full">
        <Button class="ml-auto" type="submit">
          <slot name="button">Update Organization</slot>
        </Button>
      </div>
    </Card.Footer>
  </form>
</div>
<SuperDebug data={$formData} />
