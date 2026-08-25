<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { SButton } from '@/ui/components/button'
import { SIcon } from '@/ui/components/icon'
import { SSeparator } from '@/ui/components/separator'
import { SBadge } from '@/ui/components/badge'
import { useDeviceStore } from '@/stores/device'

const router = useRouter()
const route = useRoute()
const deviceStore = useDeviceStore()
const showMobileMenu = ref(false)

const menus = [
  { label: '传文件', icon: 'lucide:upload', path: '/' },
  { label: '附近设备', icon: 'lucide:scan-search', path: '/devices' },
  { label: '传输记录', icon: 'lucide:history', path: '/history' },
  { label: '设置', icon: 'lucide:settings', path: '/settings' },
]

function navTo(path: string) {
  router.push(path)
  showMobileMenu.value = false
}

onMounted(() => {
  deviceStore.fetchLocal()
})
</script>

<template>
  <div class="flex h-[100dvh] w-screen overflow-hidden bg-background flex-col md:flex-row">
    <!-- Mobile header -->
    <header
      class="flex md:hidden h-[calc(3.5rem+env(safe-area-inset-top))] shrink-0 items-center justify-between px-4 border-b bg-card pt-[env(safe-area-inset-top)]"
    >
      <div class="flex items-center gap-2">
        <div
          class="size-8 rounded-lg bg-primary flex items-center justify-center text-primary-foreground"
        >
          <SIcon icon="lucide:zap" class="text-base" />
        </div>
        <span class="font-bold text-base">FlashLAN</span>
        <SBadge color="secondary" size="sm" class="text-xs">Beta</SBadge>
      </div>
      <SButton
        variant="ghost"
        size="sm"
        class="size-9 p-0"
        @click="showMobileMenu = !showMobileMenu"
      >
        <SIcon :icon="showMobileMenu ? 'lucide:x' : 'lucide:menu'" class="text-lg" />
      </SButton>
    </header>

    <!-- Mobile drawer overlay -->
    <div v-if="showMobileMenu" class="fixed inset-0 z-40 md:hidden" @click="showMobileMenu = false">
      <div class="absolute inset-0 bg-black/40" />
      <div
        class="absolute left-0 top-0 bottom-0 w-72 bg-card flex flex-col p-3 pt-[calc(0.75rem+env(safe-area-inset-top))] shadow-xl"
        @click.stop
      >
        <div class="flex items-center gap-3 px-2 py-3">
          <div
            class="size-8 rounded-lg bg-primary flex items-center justify-center text-primary-foreground"
          >
            <SIcon icon="lucide:zap" />
          </div>
          <span class="font-bold">FlashLAN</span>
        </div>
        <SSeparator class="my-2" />
        <nav class="space-y-1">
          <SButton
            v-for="item in menus"
            :key="item.path"
            :variant="route.path === item.path ? 'solid' : 'ghost'"
            :color="route.path === item.path ? 'primary' : 'secondary'"
            class="w-full justify-start"
            @click="navTo(item.path)"
          >
            <SIcon :icon="item.icon" />
            {{ item.label }}
          </SButton>
        </nav>
        <div class="mt-auto p-2">
          <div class="flex items-center gap-3 px-3 py-3 rounded-lg bg-muted">
            <div
              class="size-8 rounded-full bg-primary flex items-center justify-center text-primary-foreground text-xs font-bold"
            >
              MB
            </div>
            <div class="flex-1 min-w-0">
              <div class="text-sm font-medium truncate leading-none">
                {{ deviceStore.localDevice?.name || '本机' }}
              </div>
              <div class="text-xs text-muted-foreground font-mono truncate mt-1">
                {{ deviceStore.localDevice?.ip || '...' }}:{{
                  deviceStore.localDevice?.port || 17321
                }}
              </div>
            </div>
            <div class="size-2 rounded-full bg-success" />
          </div>
        </div>
      </div>
    </div>

    <!-- Desktop sidebar -->
    <aside class="hidden md:flex w-60 shrink-0 border-r bg-card flex-col">
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
    <main
      class="flex-1 min-w-0 overflow-auto bg-muted/20 pb-[calc(4rem+env(safe-area-inset-bottom))] md:pb-0"
    >
      <RouterView />
    </main>

    <!-- Mobile bottom nav -->
    <nav
      class="flex md:hidden fixed bottom-0 left-0 right-0 bg-card border-t px-1 py-1 pb-[env(safe-area-inset-bottom)] z-30"
    >
      <button
        v-for="item in menus"
        :key="item.path"
        class="flex-1 flex flex-col items-center gap-0.5 py-1.5 rounded-lg text-xs transition-colors"
        :class="route.path === item.path ? 'text-primary bg-primary/10' : 'text-muted-foreground'"
        @click="navTo(item.path)"
      >
        <SIcon :icon="item.icon" class="text-lg" />
        <span class="text-[10px] leading-none">{{ item.label }}</span>
      </button>
    </nav>
  </div>
</template>
