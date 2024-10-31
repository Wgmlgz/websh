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
    PersonSchema,
    type Person,
    UnitOfMeasure,
    OrganizationType,
    Color,
    Country,
    type OrganizationSchema
  } from '$lib/types';
  import BaseLabel from '$lib/components/ui/label/label.svelte';
  import moment from 'moment';
  import * as Select from '../ui/select';
  import _ from 'lodash';
  import NumberInput from '../ui/input/number-input.svelte';
  export let data: Person;
  import * as Dialog from '$lib/components/ui/dialog';
  import CalendarIcon from 'lucide-svelte/icons/calendar';
  import { DateFormatter, type DateValue, getLocalTimeZone } from '@internationalized/date';
  import { cn } from '$lib/utils.js';
  import { Calendar } from '$lib/components/ui/calendar/index.js';
  import * as Popover from '$lib/components/ui/popover/index.js';

  export let onSubmit: (data: Person) => any = (data) => {
    toast.success(`You submitted ${JSON.stringify(data, null, 2)}`);
  };

  const form = superForm(data, {
    SPA: true,
    dataType: 'json',
    validators: zodClient(PersonSchema),
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

  $: selectedEyeColor = $formData.eyeColor
    ? {
        label: _.startCase(_.toLower($formData.eyeColor)),
        value: $formData.eyeColor
      }
    : undefined;

  $: selectedHairColor = $formData.hairColor
    ? {
        label: _.startCase(_.toLower($formData.hairColor)),
        value: $formData.hairColor
      }
    : undefined;

  $: selectedNationality = $formData.nationality
    ? {
        label: _.startCase(_.toLower($formData.nationality)),
        value: $formData.nationality
      }
    : undefined;

  const df = new DateFormatter('en-US', {
    dateStyle: 'long'
  });

  let birthday: DateValue | undefined = undefined;
  $: if (birthday) {
    $formData.birthday = birthday.toDate(getLocalTimeZone()).toISOString() as any;
  }
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
        <!-- Owner -->
        <div>
          <BaseLabel>Owner</BaseLabel>
          <Field {form} name="name">
            <Control let:attrs>
              <Label>Name</Label>
              <Input {...attrs} bind:value={$formData.name} />
            </Control>
            <FieldErrors />
          </Field>

          <!-- Owner Eye Color -->
          <Field {form} name="eyeColor">
            <Control let:attrs>
              <Label>Eye Color</Label>
              <Select.Root
                portal={null}
                selected={selectedEyeColor}
                onSelectedChange={(v) => {
                  v && ($formData.eyeColor = v.value);
                }}
              >
                <Select.Trigger>
                  <Select.Value placeholder="Select Eye Color" />
                </Select.Trigger>
                <Select.Content>
                  <Select.Group>
                    <Select.Label>Colors</Select.Label>
                    {#each Color.options as color}
                      {@const label = _.startCase(_.toLower(color))}
                      <Select.Item value={color} {label}>
                        {label}
                      </Select.Item>
                    {/each}
                  </Select.Group>
                </Select.Content>
              </Select.Root>
            </Control>
            <FieldErrors />
          </Field>
          <!-- Owner Hair Color -->
          <Field {form} name="hairColor">
            <Control let:attrs>
              <Label>Hair Color</Label>
              <Select.Root
                portal={null}
                selected={selectedHairColor}
                onSelectedChange={(v) => {
                  v && ($formData.hairColor = v.value);
                }}
              >
                <Select.Trigger>
                  <Select.Value placeholder="Select Hair Color" />
                </Select.Trigger>
                <Select.Content>
                  <Select.Group>
                    <Select.Label>Colors</Select.Label>
                    {#each Color.options as color}
                      {@const label = _.startCase(_.toLower(color))}
                      <Select.Item value={color} {label}>
                        {label}
                      </Select.Item>
                    {/each}
                  </Select.Group>
                </Select.Content>
              </Select.Root>
            </Control>
            <FieldErrors />
          </Field>
          <!-- Owner Nationality -->
          <Field {form} name="nationality">
            <Control let:attrs>
              <Label>Nationality</Label>
              <Select.Root
                portal={null}
                selected={selectedNationality}
                onSelectedChange={(v) => {
                  v && ($formData.nationality = v.value);
                }}
              >
                <Select.Trigger>
                  <Select.Value placeholder="Select Nationality" />
                </Select.Trigger>
                <Select.Content>
                  <Select.Group>
                    <Select.Label>Countries</Select.Label>
                    {#each Country.options as country}
                      {@const label = _.startCase(_.toLower(country))}
                      <Select.Item value={country} {label}>
                        {label}
                      </Select.Item>
                    {/each}
                  </Select.Group>
                </Select.Content>
              </Select.Root>
            </Control>
            <FieldErrors />
          </Field>
          <!-- Owner Birthday -->
          <Field {form} name="birthday">
            <Control let:attrs>
              <Label>Birthday</Label>
              <!-- <Input type="date" {...attrs} bind:value={$formData.birthday} /> -->

              <Popover.Root>
                <Popover.Trigger asChild let:builder>
                  <Button
                    variant="outline"
                    class={cn(
                      'w-[280px] justify-start text-left font-normal',
                      !birthday && 'text-muted-foreground'
                    )}
                    builders={[builder]}
                  >
                    <CalendarIcon class="mr-2 h-4 w-4" />
                    {birthday ? df.format(birthday.toDate(getLocalTimeZone())) : 'Pick birthday'}
                  </Button>
                </Popover.Trigger>
                <Popover.Content class="w-auto p-0">
                  <Calendar bind:value={birthday} initialFocus />
                </Popover.Content>
              </Popover.Root>
            </Control>
            <FieldErrors />
          </Field>
          <!-- Owner Location -->
          <BaseLabel>Owner Location</BaseLabel>
          <div class="gap-10px grid grid-cols-3">
            <Field {form} name="location.x">
              <Control let:attrs>
                <Label>x</Label>
                <NumberInput {...attrs} bind:value={$formData.location.x} />
              </Control>
              <FieldErrors />
            </Field>
            <Field {form} name="location.y">
              <Control let:attrs>
                <Label>y</Label>
                <NumberInput {...attrs} bind:value={$formData.location.y} />
              </Control>
              <FieldErrors />
            </Field>
            <Field {form} name="location.z">
              <Control let:attrs>
                <Label>z</Label>
                <NumberInput {...attrs} bind:value={$formData.location.z} />
              </Control>
              <FieldErrors />
            </Field>
          </div>
        </div>
      </div>
    </Card.Content>
    <Card.Footer>
      <div class="flex w-full">
        <Button class="ml-auto" type="submit">
          <slot name="button">Update</slot>
        </Button>
      </div>
    </Card.Footer>
  </form>
</div>
<!-- <SuperDebug data={$formData} /> -->
