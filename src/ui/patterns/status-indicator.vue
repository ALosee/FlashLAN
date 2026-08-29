<script setup lang="ts">
import { computed } from 'vue'
import type { StatusTone } from './types'

defineOptions({
  name: 'StatusIndicator',
})

const props = withDefaults(
  defineProps<{
    label: string
    tone?: StatusTone
    pulse?: boolean
    live?: 'off' | 'polite' | 'assertive'
  }>(),
  {
    tone: 'neutral',
    pulse: false,
    live: 'off',
  },
)

const toneClasses: Record<StatusTone, { text: string; dot: string }> = {
  neutral: { text: 'text-muted-foreground', dot: 'bg-muted-foreground' },
  primary: { text: 'text-primary', dot: 'bg-primary' },
  info: { text: 'text-info', dot: 'bg-info' },
  success: { text: 'text-success', dot: 'bg-success' },
  warning: { text: 'text-warning', dot: 'bg-warning' },
  destructive: { text: 'text-destructive', dot: 'bg-destructive' },
}

const classes = computed(() => toneClasses[props.tone])
</script>

<template>
  <span
    class="inline-flex items-center gap-2 text-xs font-medium"
    :class="classes.text"
    :role="live === 'off' ? undefined : 'status'"
    :aria-live="live === 'off' ? undefined : live"
  >
    <span
      aria-hidden="true"
      class="size-1.5 shrink-0 rounded-full"
      :class="[classes.dot, pulse ? 'animate-pulse' : '']"
    />
    <span>{{ label }}</span>
  </span>
</template>
