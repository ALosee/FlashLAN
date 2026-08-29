<script setup lang="ts">
import { computed } from 'vue'
import { SIcon } from '@/ui/components/icon'
import StatusIndicator from './status-indicator.vue'
import type { StatusTone } from './types'

defineOptions({
  name: 'TransferRow',
})

const props = withDefaults(
  defineProps<{
    direction: 'send' | 'receive'
    fileName: string
    peer: string
    size: string
    time?: string
    status: string
    tone?: StatusTone
    path?: string
    pathTitle?: string
    error?: string
  }>(),
  {
    time: '',
    tone: 'neutral',
    path: '',
    pathTitle: '',
    error: '',
  },
)

const directionIcon = computed(() =>
  props.direction === 'receive' ? 'lucide:download' : 'lucide:upload',
)
</script>

<template>
  <article class="flex min-h-16 items-start gap-3 px-3 py-3 transition-colors duration-150 hover:bg-muted/30 sm:px-4">
    <div
      class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground"
      aria-hidden="true"
    >
      <SIcon :icon="directionIcon" class="text-base" />
    </div>

    <div class="min-w-0 flex-1">
      <div class="flex items-start justify-between gap-3">
        <div class="min-w-0">
          <h3 class="truncate text-sm font-semibold" :title="fileName">{{ fileName }}</h3>
          <div class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
            <span>{{ direction === 'receive' ? '接收自' : '发送至' }} {{ peer }}</span>
            <span aria-hidden="true">·</span>
            <span>{{ size }}</span>
            <template v-if="time">
              <span aria-hidden="true">·</span>
              <span>{{ time }}</span>
            </template>
          </div>
        </div>
        <StatusIndicator class="shrink-0" :label="status" :tone="tone" />
      </div>

      <div v-if="path || $slots.actions" class="mt-2 flex min-w-0 items-center gap-2">
        <div
          v-if="path"
          class="flex min-w-0 flex-1 items-center gap-2 text-xs text-muted-foreground"
          :title="pathTitle || path"
        >
          <SIcon icon="lucide:folder" class="shrink-0" />
          <span class="truncate">{{ path }}</span>
        </div>
        <div v-else class="min-w-0 flex-1" />
        <div v-if="$slots.actions" class="flex shrink-0 items-center gap-2">
          <slot name="actions" />
        </div>
      </div>

      <div v-if="error" class="mt-2 flex min-w-0 items-center gap-2 text-xs text-destructive">
        <SIcon icon="lucide:circle-alert" class="shrink-0" />
        <span class="truncate" :title="error">{{ error }}</span>
      </div>
    </div>
  </article>
</template>
