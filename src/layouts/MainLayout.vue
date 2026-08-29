<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { SButton } from '@/ui/components/button'
import { SIcon } from '@/ui/components/icon'
import { SSeparator } from '@/ui/components/separator'
import { AppSidebar, StatusIndicator } from '@/ui/patterns'
import type { AppNavigationItem } from '@/ui/patterns'
import { useDeviceStore } from '@/stores/device'
import { useTransferStore } from '@/stores/transfer'

const router = useRouter()
const route = useRoute()
const deviceStore = useDeviceStore()
const transferStore = useTransferStore()
const showMobileMenu = ref(false)
const respondingTaskId = ref('')

const menus: AppNavigationItem[] = [
  { label: '传文件', icon: 'lucide:upload', path: '/' },
  { label: '消息', icon: 'lucide:messages-square', path: '/messages' },
  { label: '附近设备', icon: 'lucide:scan-search', path: '/devices' },
  { label: '传输记录', icon: 'lucide:history', path: '/history' },
  { label: '设置', icon: 'lucide:settings', path: '/settings' },
]

function navTo(path: string) {
  router.push(path)
  showMobileMenu.value = false
}

const activeTasks = computed(() =>
  transferStore.tasks.filter(task => task.status === 'transferring' || task.status === 'pending'),
)
const pendingRequest = computed(() => transferStore.pendingRequests[0])
const pendingQueueCount = computed(() => Math.max(transferStore.pendingRequests.length - 1, 0))

const avatarText = computed(() => {
  const name = deviceStore.localDevice?.name?.trim()
  if (!name) return 'FL'
  const chars = Array.from(name.replace(/\s+/g, '')).slice(0, 2)
  return chars.map(char => char.toUpperCase()).join('') || 'FL'
})

const localDeviceAddress = computed(() => {
  const ip = deviceStore.localDevice?.ip || '获取中...'
  const port = deviceStore.localDevice?.port || 17321
  return [ip, port].join(':')
})

function formatBytes(bytes: number) {
  if (!bytes) return '0 B'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`
}

async function respondToRequest(taskId: string, accepted: boolean) {
  respondingTaskId.value = taskId
  try {
    await transferStore.respondToRequest(taskId, accepted)
  } catch (error) {
    console.error('[FlashLAN] transfer request response failed', error)
  } finally {
    respondingTaskId.value = ''
  }
}

onMounted(async () => {
  deviceStore.fetchLocal()
  await transferStore.initialize()
})
</script>

<template>
  <div class="flex h-[100dvh] w-screen flex-col overflow-hidden bg-background md:flex-row">
    <!-- Mobile header -->
    <header
      class="flex h-[calc(3.5rem+env(safe-area-inset-top))] shrink-0 items-center justify-between border-b border-border bg-card px-4 pt-[env(safe-area-inset-top)] md:hidden"
    >
      <div class="flex items-center gap-2">
        <img
          src="/flashlan-icon.svg"
          alt=""
          aria-hidden="true"
          class="size-8 shrink-0 rounded-lg"
        />
        <span class="text-base font-bold">FlashLAN</span>
        <span class="rounded-md bg-muted px-2 py-1 text-xs text-muted-foreground">Beta</span>
      </div>
      <SButton
        variant="ghost"
        size="lg"
        shape="square"
        class="size-11 p-0"
        :aria-label="showMobileMenu ? '关闭导航' : '打开导航'"
        :title="showMobileMenu ? '关闭导航' : '打开导航'"
        @click="showMobileMenu = !showMobileMenu"
      >
        <SIcon :icon="showMobileMenu ? 'lucide:x' : 'lucide:menu'" class="text-lg" />
      </SButton>
    </header>

    <!-- Mobile drawer overlay -->
    <Transition name="fl-drawer">
      <div
        v-if="showMobileMenu"
        class="fixed inset-0 z-40 md:hidden"
        @click="showMobileMenu = false"
      >
        <div class="absolute inset-0 bg-black/40" />
        <div
          class="fl-drawer-panel absolute bottom-0 left-0 top-0 flex w-72 flex-col bg-card p-3 pt-[calc(0.75rem+env(safe-area-inset-top))] shadow-xl"
          @click.stop
        >
          <div class="flex items-center gap-3 px-2 py-3">
            <img
              src="/flashlan-icon.svg"
              alt=""
              aria-hidden="true"
              class="size-8 shrink-0 rounded-lg"
            />
            <span class="font-bold">FlashLAN</span>
          </div>
          <SSeparator class="my-2" />
          <nav class="space-y-1">
            <SButton
              v-for="item in menus"
              :key="item.path"
              :variant="route.path === item.path ? 'soft' : 'ghost'"
              :color="route.path === item.path ? 'primary' : 'secondary'"
              class="h-11 w-full justify-start"
              :aria-current="route.path === item.path ? 'page' : undefined"
              @click="navTo(item.path)"
            >
              <SIcon :icon="item.icon" />
              {{ item.label }}
            </SButton>
          </nav>
          <div class="mt-auto p-2">
            <div class="flex items-center gap-3 rounded-lg bg-muted/60 px-3 py-3">
              <div
                class="flex size-8 shrink-0 items-center justify-center rounded-full bg-primary/10 text-xs font-bold text-primary"
              >
                {{ avatarText }}
              </div>
              <div class="min-w-0 flex-1">
                <div class="truncate text-sm font-medium leading-none">
                  {{ deviceStore.localDevice?.name || '本机' }}
                </div>
                <div
                  class="mt-1 truncate font-mono text-xs text-muted-foreground"
                  :title="localDeviceAddress"
                >
                  {{ localDeviceAddress }}
                </div>
              </div>
              <StatusIndicator label="在线" tone="success" />
            </div>
          </div>
        </div>
      </div>
    </Transition>

    <!-- Desktop sidebar -->
    <AppSidebar
      :items="menus"
      :active-path="route.path"
      :device-name="deviceStore.localDevice?.name || '本机'"
      :device-address="localDeviceAddress"
      :avatar-text="avatarText"
      @navigate="navTo"
    />

    <!-- Main -->
    <main
      class="flex-1 min-h-0 min-w-0 bg-muted/20 pb-[calc(3rem+env(safe-area-inset-bottom))] md:pb-0"
      :class="route.path === '/messages' ? 'overflow-hidden' : 'overflow-auto'"
    >
      <RouterView v-slot="{ Component }">
        <Transition name="fl-page-view" mode="out-in">
          <component :is="Component" :key="route.path" />
        </Transition>
      </RouterView>
    </main>

    <!-- Incoming file confirmation. The server waits for this decision before sending bytes. -->
    <Transition name="fl-dialog">
      <div
        v-if="pendingRequest"
        class="fixed inset-0 z-50 flex items-center justify-center p-4"
        role="dialog"
        aria-modal="true"
      >
        <div class="absolute inset-0 bg-black/45 backdrop-blur-sm" />
        <div
          class="fl-dialog-panel relative w-full max-w-sm rounded-xl border border-border bg-card p-4 shadow-2xl"
        >
          <div class="flex items-start gap-3">
            <div
              class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary"
            >
              <SIcon icon="lucide:download" class="text-base" />
            </div>
            <div class="min-w-0">
              <h2 class="font-semibold">
                收到文件
                <span
                  v-if="pendingQueueCount"
                  class="ml-2 inline-flex items-center rounded-full bg-primary/10 px-2 py-1 align-middle text-xs font-medium text-primary"
                >
                  还有 {{ pendingQueueCount }} 个待确认
                </span>
              </h2>
              <p class="text-sm text-muted-foreground mt-1 truncate">
                {{ pendingRequest.peer }} 想向本机发送文件
              </p>
            </div>
          </div>
          <div class="mt-4 rounded-xl bg-muted/60 p-3">
            <div class="font-medium truncate">{{ pendingRequest.fileName }}</div>
            <div class="text-xs text-muted-foreground mt-1">
              <template v-if="pendingRequest.fileCount > 1">
                共 {{ pendingRequest.fileCount }} 个文件 ·
              </template>
              {{ formatBytes(pendingRequest.total) }} · 文件将在确认后开始传输
            </div>
          </div>
          <div class="mt-4 flex gap-3">
            <SButton
              variant="outline"
              class="flex-1"
              :disabled="respondingTaskId === pendingRequest.taskId"
              @click="respondToRequest(pendingRequest.taskId, false)"
            >
              拒绝
            </SButton>
            <SButton
              class="flex-1"
              :loading="respondingTaskId === pendingRequest.taskId"
              @click="respondToRequest(pendingRequest.taskId, true)"
            >
              接收文件
            </SButton>
          </div>
        </div>
      </div>
    </Transition>

    <!-- Keep incoming and outgoing transfers visible without requiring a page switch. -->
    <Transition name="fl-float">
      <div
        v-if="activeTasks.length && route.path !== '/'"
        class="fixed bottom-[calc(4.75rem+env(safe-area-inset-bottom))] right-3 z-40 w-88 max-w-[calc(100vw-1.5rem)] md:bottom-6 md:right-6"
      >
        <button
          class="w-full rounded-xl border border-border bg-card/95 p-3 text-left shadow-xl outline-none backdrop-blur transition-colors hover:bg-muted/50 focus-visible:ring-3 focus-visible:ring-primary/30"
          @click="navTo('/')"
        >
          <div class="flex items-center justify-between gap-3">
            <span class="text-sm font-semibold flex items-center gap-2">
              <SIcon icon="lucide:arrow-down-to-line" class="text-primary" />
              {{ activeTasks.length }} 个文件正在传输
            </span>
            <SIcon icon="lucide:chevron-right" class="text-muted-foreground" />
          </div>
          <div class="mt-2 space-y-2">
            <div v-for="task in activeTasks.slice(0, 2)" :key="task.id" class="min-w-0">
              <div class="flex items-center justify-between gap-2 text-xs">
                <span class="truncate">{{ task.fileName }}</span>
                <span class="shrink-0 text-muted-foreground">{{ task.progress.toFixed(0) }}%</span>
              </div>
              <div class="h-1.5 rounded-full bg-muted overflow-hidden mt-1">
                <div
                  class="h-full bg-primary rounded-full transition-all"
                  :style="{ width: `${task.progress}%` }"
                />
              </div>
              <div class="mt-1 text-xs text-muted-foreground">
                {{ task.direction === 'receive' ? '接收中' : '发送中' }} ·
                {{ formatBytes(task.transferred) }}
                <span v-if="task.total">/ {{ formatBytes(task.total) }}</span>
              </div>
            </div>
          </div>
        </button>
      </div>
    </Transition>

    <!-- Mobile bottom nav -->
    <nav
      class="fixed bottom-0 left-0 right-0 z-30 flex border-t border-border bg-card px-1 py-1 pb-[env(safe-area-inset-bottom)] md:hidden"
    >
      <button
        v-for="item in menus"
        :key="item.path"
        class="flex min-h-11 flex-1 flex-col items-center justify-center gap-1 rounded-lg px-1 text-xs transition-colors"
        :class="route.path === item.path ? 'text-primary bg-primary/10' : 'text-muted-foreground'"
        @click="navTo(item.path)"
      >
        <SIcon :icon="item.icon" class="text-lg" />
        <span class="text-xs leading-none">{{ item.label }}</span>
      </button>
    </nav>
  </div>
</template>
