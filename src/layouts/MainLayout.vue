<script setup lang="ts">
import { onMounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { SButton } from '@/ui/components/button'
import { SIcon } from '@/ui/components/icon'
import { SSeparator } from '@/ui/components/separator'
import { SBadge } from '@/ui/components/badge'
import { useDeviceStore } from '@/stores/device'

const router = useRouter()
const route = useRoute()
const deviceStore = useDeviceStore()

const menus = [
  { label: '传文件', icon: 'lucide:upload', path: '/' },
  { label: '附近设备', icon: 'lucide:scan-search', path: '/devices' },
  { label: '传输记录', icon: 'lucide:history', path: '/history' },
  { label: '设置', icon: 'lucide:settings', path: '/settings' },
]

function navTo(path: string) {
  router.push(path)
}

onMounted(() => {
  deviceStore.fetchLocal()
})
</script>

<template>
  <div class="flex h-screen w-screen overflow-hidden bg-background">
    <!-- Sidebar -->
    <aside class="w-60 shrink-0 border-r bg-card flex flex-col">
      <div class="h-14 flex items-center gap-3 px-5 shrink-0">
        <div
          class="size-8 rounded-lg bg-primary flex items-center justify-center text-primary-foreground shrink-0"
        >
          <SIcon icon="lucide:zap" class="text-base" />
        </div>
        <span class="font-bold text-base tracking-tight">FlashLAN</span>
        <SBadge color="secondary" size="sm" class="ml-1 text-xs">Beta</SBadge>
      </div>

      <SSeparator />

      <nav class="flex-1 p-3 space-y-1 overflow-y-auto">
        <SButton
          v-for="item in menus"
          :key="item.path"
          :variant="route.path === item.path ? 'solid' : 'ghost'"
          :color="route.path === item.path ? 'primary' : 'secondary'"
          class="w-full justify-start font-medium"
          @click="navTo(item.path)"
        >
          <SIcon :icon="item.icon" class="text-base shrink-0" />
          {{ item.label }}
        </SButton>
      </nav>

      <SSeparator />

      <!-- Device info footer -->
      <div class="p-3 shrink-0">
        <div class="flex items-center gap-3 px-3 py-3 rounded-lg bg-muted">
          <div
            class="size-8 rounded-full bg-primary flex items-center justify-center text-primary-foreground text-xs font-bold shrink-0"
          >
            MB
          </div>
          <div class="flex-1 min-w-0">
            <div class="text-sm font-medium truncate leading-none">
              {{ deviceStore.localDevice?.name || '本机' }}
            </div>
            <div class="text-xs text-muted-foreground truncate mt-1 font-mono">
              {{ deviceStore.localDevice?.ip || '获取中...' }}:{{
                deviceStore.localDevice?.port || 17321
              }}
            </div>
          </div>
          <div class="size-2 rounded-full bg-success shrink-0" />
        </div>
      </div>
    </aside>

    <!-- Main -->
    <main class="flex-1 min-w-0 overflow-auto bg-muted/20">
      <RouterView />
    </main>
  </div>
</template>
