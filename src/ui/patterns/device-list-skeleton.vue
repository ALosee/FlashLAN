<script setup lang="ts">
import { SSkeleton } from '@/ui/components/skeleton'

defineOptions({
  name: 'DeviceListSkeleton',
})

withDefaults(
  defineProps<{
    rows?: number
    label?: string
  }>(),
  {
    rows: 3,
    label: '正在刷新设备列表',
  },
)
</script>

<template>
  <section class="space-y-3" role="status" aria-live="polite" aria-busy="true">
    <span class="sr-only">{{ label }}</span>

    <div class="flex items-center gap-2">
      <SSkeleton class="size-8 shrink-0" />
      <SSkeleton class="h-4 w-24" />
      <SSkeleton class="ml-auto h-6 w-12" />
    </div>

    <div class="divide-y divide-border overflow-hidden rounded-lg border border-border bg-card">
      <div
        v-for="row in rows"
        :key="row"
        class="flex min-h-18 items-center gap-3 px-3 py-2 sm:min-h-20 sm:px-4 sm:py-3"
      >
        <SSkeleton class="size-8 shrink-0" />
        <div class="min-w-0 flex-1">
          <SSkeleton class="h-4 w-32 max-w-full" />
          <SSkeleton class="mt-2 h-3 w-48 max-w-full" />
        </div>
        <SSkeleton class="h-6 w-12 shrink-0" />
      </div>
    </div>
  </section>
</template>
