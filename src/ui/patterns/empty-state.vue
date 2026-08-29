<script setup lang="ts">
import { SButton } from '@/ui/components/button'
import { SIcon } from '@/ui/components/icon'

defineOptions({
  name: 'EmptyState',
})

withDefaults(
  defineProps<{
    icon: string
    title: string
    description: string
    actionLabel?: string
    compact?: boolean
  }>(),
  {
    actionLabel: '',
    compact: false,
  },
)

const emit = defineEmits<{
  action: []
}>()
</script>

<template>
  <div
    class="flex flex-col items-center justify-center px-4 text-center"
    :class="compact ? 'min-h-20 py-3' : 'min-h-40 py-6'"
  >
    <div
      class="flex size-8 items-center justify-center rounded-lg bg-muted text-muted-foreground"
      aria-hidden="true"
    >
      <SIcon :icon="icon" class="text-base" />
    </div>
    <h2 class="mt-3 text-sm font-semibold">{{ title }}</h2>
    <p class="mt-1 max-w-md text-xs text-muted-foreground">{{ description }}</p>
    <SButton v-if="actionLabel" variant="outline" size="sm" class="mt-3 min-h-11 sm:min-h-8" @click="emit('action')">
      {{ actionLabel }}
    </SButton>
  </div>
</template>
