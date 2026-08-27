<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { SButton } from '@/ui/components/button'
import { SIcon } from '@/ui/components/icon'
import { SSeparator } from '@/ui/components/separator'
import { SBadge } from '@/ui/components/badge'
import { useDeviceStore } from '@/stores/device'
import { useTransferStore } from '@/stores/transfer'

const router = useRouter()
const route = useRoute()
const deviceStore = useDeviceStore()
const transferStore = useTransferStore()
const showMobileMenu = ref(false)
const respondingTaskId = ref('')

const menus = [
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
  <div class="flex h-[100dvh] w-screen overflow-hidden bg-background flex-col md:flex-row">
    <!-- Mobile header -->
    <header
      class="flex md:hidden h-[calc(3.5rem+env(safe-area-inset-top))] shrink-0 items-center justify-between px-4 border-b bg-card pt-[env(safe-area-inset-top)]"
    >
      <div class="flex items-center gap-2">
        <img
          src="/flashlan-icon.svg"
          alt=""
          aria-hidden="true"
          class="size-8 rounded-lg shrink-0"
        />
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
          <img
            src="/flashlan-icon.svg"
            alt=""
            aria-hidden="true"
            class="size-8 rounded-lg shrink-0"
          />
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
              {{ avatarText }}
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
        <img
          src="/flashlan-icon.svg"
          alt=""
          aria-hidden="true"
          class="size-8 rounded-lg shrink-0"
        />
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
            {{ avatarText }}
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

    <!-- Incoming file confirmation. The server waits for this decision before sending bytes. -->
    <div
      v-if="pendingRequest"
      class="fixed inset-0 z-50 flex items-center justify-center p-4"
      role="dialog"
      aria-modal="true"
    >
      <div class="absolute inset-0 bg-black/45 backdrop-blur-[2px]" />
      <div class="relative w-full max-w-sm rounded-2xl border bg-card shadow-2xl p-5">
        <div class="flex items-start gap-3">
          <div
            class="size-11 rounded-xl bg-primary/10 text-primary flex items-center justify-center shrink-0"
          >
            <SIcon icon="lucide:download" class="text-xl" />
          </div>
          <div class="min-w-0">
            <h2 class="font-semibold">
              收到文件
              <span
                v-if="pendingQueueCount"
                class="ml-1.5 inline-flex items-center rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary align-middle"
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
        <div class="mt-5 flex gap-3">
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

    <!-- Keep incoming and outgoing transfers visible without requiring a page switch. -->
    <div
      v-if="activeTasks.length && route.path !== '/'"
      class="fixed right-3 bottom-[calc(4.75rem+env(safe-area-inset-bottom))] md:right-5 md:bottom-5 z-40 w-[min(22rem,calc(100vw-1.5rem))]"
    >
      <button
        class="w-full text-left rounded-xl border bg-card/95 backdrop-blur shadow-xl p-3 transition-colors hover:bg-muted/50"
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
            <div class="text-[11px] text-muted-foreground mt-1">
              {{ task.direction === 'receive' ? '接收中' : '发送中' }} ·
              {{ formatBytes(task.transferred) }}
              <span v-if="task.total">/ {{ formatBytes(task.total) }}</span>
            </div>
          </div>
        </div>
      </button>
    </div>

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
