<script setup lang="ts">
import { SIcon } from '@/ui/components/icon'
import StatusIndicator from './status-indicator.vue'

defineOptions({
  name: 'DeviceRow',
})

withDefaults(
  defineProps<{
    icon: string
    name: string
    address: string
    platform?: string
    source?: string
    online?: boolean
    trusted?: boolean
    current?: boolean
  }>(),
  {
    platform: '',
    source: '',
    online: true,
    trusted: false,
    current: false,
  },
)
</script>

<template>
  <article class="flex min-h-18 items-center gap-3 px-3 py-2 transition-colors duration-150 hover:bg-muted/30 sm:min-h-20 sm:px-4 sm:py-3">
    <div class="flex min-w-0 flex-1 items-start gap-3">
      <div
        class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground"
        aria-hidden="true"
      >
        <SIcon :icon="icon" class="text-base" />
      </div>

      <div class="min-w-0 flex-1">
        <div class="flex min-w-0 items-center gap-2">
          <h3 class="truncate text-sm font-semibold" :title="name">{{ name }}</h3>
          <span
            v-if="current"
            class="shrink-0 rounded-md bg-muted px-2 py-1 text-xs font-medium text-muted-foreground"
          >
            当前设备
          </span>
        </div>
        <div class="mt-1 flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
          <span class="max-w-full truncate font-mono" :title="address">{{ address }}</span>
          <template v-if="platform">
            <span aria-hidden="true">·</span>
            <span>{{ platform }}</span>
          </template>
          <template v-if="source">
            <span aria-hidden="true">·</span>
            <span>{{ source }}</span>
          </template>
        </div>
        <div v-if="$slots.detail" class="mt-2">
          <slot name="detail" />
        </div>
      </div>
    </div>

    <div class="flex min-h-8 shrink-0 items-center gap-2">
      <div class="flex flex-col items-end gap-1 sm:flex-row sm:items-center sm:gap-3">
        <StatusIndicator v-if="trusted" label="可信" tone="success" />
        <StatusIndicator :label="online ? '在线' : '离线'" :tone="online ? 'success' : 'neutral'" />
      </div>
      <div v-if="$slots.actions" class="flex shrink-0 items-center gap-2">
        <slot name="actions" />
      </div>
    </div>
  </article>
</template>
