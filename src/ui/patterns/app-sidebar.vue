<script setup lang="ts">
import { SButton } from '@/ui/components/button'
import { SIcon } from '@/ui/components/icon'
import { SSeparator } from '@/ui/components/separator'
import StatusIndicator from './status-indicator.vue'
import type { AppNavigationItem } from './types'

defineOptions({
  name: 'AppSidebar',
})

withDefaults(
  defineProps<{
    items: AppNavigationItem[]
    activePath: string
    deviceName: string
    deviceAddress: string
    avatarText: string
    online?: boolean
  }>(),
  {
    online: true,
  },
)

const emit = defineEmits<{
  navigate: [path: string]
}>()
</script>

<template>
  <aside class="fl-sidebar-width hidden shrink-0 flex-col border-r border-border bg-card md:flex">
    <div class="flex h-14 shrink-0 items-center gap-3 px-4">
      <img
        src="/flashlan-icon.svg"
        alt=""
        aria-hidden="true"
        class="size-8 shrink-0 rounded-lg"
      />
      <span class="font-bold tracking-tight">FlashLAN</span>
      <span class="rounded-md bg-muted px-2 py-1 text-xs text-muted-foreground">Beta</span>
    </div>

    <SSeparator />

    <nav class="flex-1 space-y-1 overflow-y-auto p-3" aria-label="主要导航">
      <SButton
        v-for="item in items"
        :key="item.path"
        :variant="activePath === item.path ? 'soft' : 'ghost'"
        :color="activePath === item.path ? 'primary' : 'secondary'"
        class="w-full justify-start font-medium"
        :aria-current="activePath === item.path ? 'page' : undefined"
        @click="emit('navigate', item.path)"
      >
        <SIcon :icon="item.icon" class="text-base" />
        {{ item.label }}
      </SButton>
    </nav>

    <SSeparator />

    <div class="shrink-0 p-3">
      <div class="flex items-center gap-3 rounded-lg bg-muted/60 px-3 py-3">
        <div
          class="flex size-8 shrink-0 items-center justify-center rounded-full bg-primary/10 text-xs font-bold text-primary"
        >
          {{ avatarText }}
        </div>
        <div class="min-w-0 flex-1">
          <div class="truncate text-sm font-medium leading-none">{{ deviceName }}</div>
          <div class="mt-1 truncate font-mono text-xs text-muted-foreground" :title="deviceAddress">
            {{ deviceAddress }}
          </div>
          <StatusIndicator
            class="mt-2"
            :label="online ? '在线' : '离线'"
            :tone="online ? 'success' : 'neutral'"
          />
        </div>
      </div>
    </div>
  </aside>
</template>
