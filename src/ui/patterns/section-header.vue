<script setup lang="ts">
import { SIcon } from '@/ui/components/icon'

defineOptions({
  name: 'SectionHeader',
})

defineProps<{
  id?: string
  icon?: string
  title: string
  count?: string
}>()
</script>

<template>
  <header class="flex min-w-0 items-center gap-2">
    <SIcon
      v-if="icon"
      :icon="icon"
      class="shrink-0 text-muted-foreground"
      :aria-hidden="true"
    />
    <h2 :id="id" class="truncate text-base font-semibold">{{ title }}</h2>
    <span v-if="count" class="shrink-0 text-xs text-muted-foreground">{{ count }}</span>
    <div v-if="$slots.actions" class="ml-auto flex shrink-0 items-center gap-2">
      <slot name="actions" />
    </div>
  </header>
</template>
