<script setup lang="ts">
import { SIcon } from '@/ui/components/icon'

defineOptions({
  name: 'SettingsRow',
})

withDefaults(
  defineProps<{
    icon: string
    title: string
    description?: string
    stacked?: boolean
  }>(),
  {
    description: '',
    stacked: false,
  },
)
</script>

<template>
  <div
    class="flex min-h-16 gap-3 px-4 py-2 transition-colors duration-150 hover:bg-muted/30"
    :class="
      stacked
        ? 'flex-col items-stretch justify-center sm:flex-row sm:items-center sm:justify-between'
        : 'items-center justify-between'
    "
  >
    <div class="flex min-w-0 items-center gap-3">
      <div
        class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground"
        aria-hidden="true"
      >
        <SIcon :icon="icon" class="text-base" />
      </div>
      <div class="min-w-0">
        <div class="text-sm font-medium">{{ title }}</div>
        <div v-if="description || $slots.description" class="mt-1 text-xs text-muted-foreground">
          <slot name="description">{{ description }}</slot>
        </div>
      </div>
    </div>

    <div
      v-if="$slots.control"
      class="flex shrink-0 items-center gap-2"
      :class="stacked ? 'w-full sm:w-auto' : 'w-auto'"
    >
      <slot name="control" />
    </div>
  </div>
</template>
