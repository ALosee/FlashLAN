<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { open } from '@tauri-apps/plugin-dialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { SButton } from '@/ui/components/button'
import { SIcon } from '@/ui/components/icon'
import { SSkeleton } from '@/ui/components/skeleton'
import { EmptyState, PageHeader, StatusIndicator } from '@/ui/patterns'
import { isMobilePlatform, isTauri } from '@/utils/tauri'
import { useDeviceStore } from '@/stores/device'
import { type TransferTask, useTransferStore } from '@/stores/transfer'

const deviceStore = useDeviceStore()
const transferStore = useTransferStore()
const router = useRouter()

const isDragging = ref(false)
const selectedFiles = ref<string[]>([])
const isMobile = isMobilePlatform()
const serverListening = ref(true)
const serverError = ref('')
const isSendingFiles = ref(false)
const selectedDeviceId = ref('')
const sendError = ref('')

const activeTasks = computed(() =>
  transferStore.tasks.filter(task => task.status === 'transferring' || task.status === 'pending'),
)
const finishedTasks = computed(() =>
  transferStore.tasks.filter(task => task.status === 'completed' || task.status === 'failed'),
)
const completedCount = computed(
  () => finishedTasks.value.filter(task => task.status === 'completed').length,
)
const failedCount = computed(
  () => finishedTasks.value.filter(task => task.status === 'failed').length,
)
const onlineDevices = computed(() => deviceStore.devices.filter(device => device.online !== false))
const selectedDevice = computed(
  () =>
    onlineDevices.value.find(device => device.id === selectedDeviceId.value) ||
    onlineDevices.value[0],
)
const canSendFiles = computed(() =>
  Boolean(selectedDevice.value && selectedFiles.value.length && !isSendingFiles.value),
)

watch(
  onlineDevices,
  devices => {
    if (!devices.some(device => device.id === selectedDeviceId.value)) {
      selectedDeviceId.value = devices[0]?.id || ''
    }
  },
  { immediate: true },
)

function getErrorMessage(error: unknown) {
  const message = String(error)
    .replace(/^Error:\s*/, '')
    .trim()
  return message || '连接失败，请确认设备地址和端口正确'
}

function addDroppedPaths(paths: string[]) {
  if (!paths.length) return
  const known = new Set(selectedFiles.value)
  for (const path of paths) {
    if (path && !known.has(path)) {
      selectedFiles.value.push(path)
      known.add(path)
    }
  }
}

/** Web-view level drag & drop carries real file/folder paths on desktop. */
let unlistenDragDrop: (() => void) | undefined
async function setupTauriDragDrop() {
  try {
    const unlisten = await getCurrentWebview().onDragDropEvent(event => {
      const payload = event.payload
      if (payload.type === 'enter' || payload.type === 'over') {
        isDragging.value = true
      } else if (payload.type === 'leave') {
        isDragging.value = false
      } else if (payload.type === 'drop') {
        isDragging.value = false
        addDroppedPaths(payload.paths)
      }
    })
    unlistenDragDrop = unlisten as () => void
  } catch (error) {
    console.warn('[FlashLAN] drag-drop listener unavailable', error)
  }
}

/**
 * Ctrl+V: send clipboard text as a generated .txt temp file. Images copied
 * from the system clipboard do not expose real paths, so they are out of
 * scope until a dedicated pipeline exists.
 */
function onPaste(e: ClipboardEvent) {
  const target = e.target as HTMLElement | null
  if (target?.tagName === 'INPUT' || target?.tagName === 'TEXTAREA' || target?.isContentEditable) {
    return
  }
  const text = e.clipboardData?.getData('text/plain')?.trim()
  if (!text) return
  e.preventDefault()
  void (async () => {
    try {
      if (isTauri()) {
        const path = await invoke<string>('create_text_clipboard_file', { text })
        addDroppedPaths([path])
      } else {
        selectedFiles.value.push(`clipboard-${Date.now()}.txt`)
      }
    } catch (error) {
      console.warn('[FlashLAN] paste failed', error)
    }
  })()
}

function cancelTransfer(task: TransferTask) {
  void transferStore.cancelTask(task.id)
}

async function sendFilesTo(device: { ip: string; port: number }) {
  const paths = selectedFiles.value.slice()
  if (!paths.length || isSendingFiles.value) return
  sendError.value = ''
  isSendingFiles.value = true
  try {
    await transferStore.sendFile(paths, device.ip, device.port)
    selectedFiles.value = []
  } catch (error) {
    console.error('[FlashLAN] send files failed', error)
    sendError.value = getErrorMessage(error)
  } finally {
    isSendingFiles.value = false
  }
}

function removeSelectedFile(path: string) {
  selectedFiles.value = selectedFiles.value.filter(item => item !== path)
}

/* Browser-preview fallback using standard HTML5 drag & drop. */
function onDragOver(e: DragEvent) {
  e.preventDefault()
  if (!isTauri()) isDragging.value = true
}
function onDragLeave() {
  if (!isTauri()) isDragging.value = false
}
function onDrop(e: DragEvent) {
  e.preventDefault()
  if (!isTauri()) {
    isDragging.value = false
    const files = e.dataTransfer?.files
    if (files?.length) addDroppedPaths(Array.from(files).map(file => file.name))
  }
}

async function pickFiles() {
  if (!isTauri()) {
    // Browser fallback: use hidden input
    const input = document.createElement('input')
    input.type = 'file'
    input.multiple = true
    input.addEventListener('change', () => {
      const files = input.files
      if (files) {
        addDroppedPaths(Array.from(files).map(f => f.name))
      }
    })
    input.click()
    return
  }
  try {
    const files = await open({ multiple: true, directory: false })
    if (files) {
      const list = Array.isArray(files) ? files : [files]
      addDroppedPaths(list)
    }
  } catch (e) {
    console.warn('open dialog failed', e)
  }
}

async function pickFolder() {
  if (!isTauri()) {
    const input = document.createElement('input')
    input.type = 'file'
    ;(input as HTMLInputElement & { webkitdirectory: boolean }).webkitdirectory = true
    input.addEventListener('change', () => {
      const files = input.files
      if (files && files.length > 0) {
        const first = files[0] as File & { webkitRelativePath?: string }
        addDroppedPaths([first.webkitRelativePath?.split('/')[0] || first.name])
      }
    })
    input.click()
    return
  }
  try {
    const folder = await open({ directory: true, multiple: false })
    if (folder && typeof folder === 'string') {
      addDroppedPaths([folder])
    }
  } catch (e) {
    console.warn('open folder failed', e)
  }
}

async function refreshDevices() {
  if (deviceStore.isDiscovering) return
  await deviceStore.discover()
  await deviceStore.refreshManualStatus()
}

function formatSpeed(bytesPerSec: number) {
  if (bytesPerSec < 1024) return `${bytesPerSec.toFixed(0)} B/s`
  if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(1)} KB/s`
  return `${(bytesPerSec / 1024 / 1024).toFixed(1)} MB/s`
}

function formatBytes(bytes: number) {
  if (!bytes) return '0 B'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`
}

function statusLabel(task: TransferTask) {
  if (task.status === 'completed') return '已完成'
  if (task.status === 'failed') return '失败'
  if (task.direction === 'receive') return '接收中'
  return task.status === 'transferring' ? '发送中' : '等待中'
}

function openHistory() {
  void router.push('/history')
}

function openDevices() {
  void router.push('/devices')
}

let statusTimer: ReturnType<typeof setInterval> | undefined
let unlistenServerStatus: (() => void) | undefined

onMounted(async () => {
  await deviceStore.fetchLocal()
  await deviceStore.discover()
  await transferStore.ensureListener()
  void refreshServerStatus()
  // Manual devices have no mDNS liveness; probe them periodically instead.
  await deviceStore.refreshManualStatus()
  statusTimer = setInterval(() => {
    void deviceStore.refreshManualStatus()
  }, 15000)
  if (isTauri()) {
    void setupTauriDragDrop()
    void listen<[boolean, string]>('server_status', event => {
      const [listening, message] = event.payload
      serverListening.value = listening
      serverError.value = listening ? '' : message
    }).then(unlisten => {
      unlistenServerStatus = unlisten as () => void
    })
  }
  document.addEventListener('paste', onPaste)
})

async function refreshServerStatus() {
  if (!isTauri()) return
  try {
    serverListening.value = await invoke<boolean>('get_server_status')
    serverError.value = ''
  } catch {
    serverListening.value = false
    serverError.value = '无法查询传输服务状态'
  }
}

onBeforeUnmount(() => {
  unlistenDragDrop?.()
  unlistenServerStatus?.()
  document.removeEventListener('paste', onPaste)
  if (statusTimer) clearInterval(statusTimer)
})
</script>

<template>
  <div class="fl-page fl-content-send space-y-4 md:space-y-6">
    <!-- Server bind failure banner -->
    <div
      v-if="isTauri() && !serverListening"
      class="flex items-start gap-2 rounded-xl border border-destructive/15 bg-destructive/8 px-4 py-3 text-sm text-destructive"
      role="alert"
    >
      <SIcon icon="lucide:triangle-alert" class="mt-1 shrink-0" />
      <div class="min-w-0">
        <div class="font-medium">文件接收服务未启动</div>
        <div class="mt-1 text-xs">
          {{ serverError || '端口 17321 可能被占用' }}，本机无法被其他设备发送文件，请检查后重启应用
        </div>
      </div>
    </div>

    <PageHeader title="快速传文件" description="选择文件和设备，一步发送；接收文件会自动显示在这里">
      <template #status>
        <span class="text-xs text-muted-foreground">本机可被发现</span>
        <StatusIndicator
          :label="serverListening ? '在线' : '服务异常'"
          :tone="serverListening ? 'success' : 'destructive'"
          live="polite"
        />
        <span
          v-if="deviceStore.localDevice"
          class="max-w-40 truncate rounded-md bg-muted px-2 py-1 font-mono text-xs sm:max-w-none"
          :title="[deviceStore.localDevice.ip, deviceStore.localDevice.port].join(':')"
        >
          {{ deviceStore.localDevice.ip }}:{{ deviceStore.localDevice.port }}
        </span>
      </template>
    </PageHeader>

    <!-- File upload area -->
    <section
      class="overflow-hidden rounded-lg border border-border p-2"
      :class="selectedFiles.length ? 'border-solid' : 'border-dashed'"
      aria-label="文件上传区域"
    >
      <div
        class="flex min-h-44 cursor-pointer flex-col items-center justify-center rounded-md px-4 py-4 text-center transition-colors sm:min-h-52 sm:px-8 sm:py-6"
        :class="isDragging ? 'bg-primary/5' : 'bg-card hover:bg-muted/40'"
        @dragover="onDragOver"
        @dragleave="onDragLeave"
        @drop="onDrop"
        @click="pickFiles"
      >
        <Transition name="fl-state" mode="out-in">
          <div v-if="selectedFiles.length" class="w-full max-w-xl text-left">
            <div class="flex items-center justify-between gap-3">
              <div class="flex min-w-0 items-center gap-2">
                <div
                  class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-muted text-primary"
                >
                  <SIcon icon="lucide:files" class="text-base" />
                </div>
                <div class="min-w-0">
                  <div class="text-sm font-semibold">已选择 {{ selectedFiles.length }} 项</div>
                  <div class="text-xs text-muted-foreground">可继续添加文件或文件夹</div>
                </div>
              </div>
              <SButton
                variant="ghost"
                color="destructive"
                size="sm"
                class="min-h-11 shrink-0 sm:min-h-8"
                @click.stop="selectedFiles = []"
              >
                清空
              </SButton>
            </div>
            <TransitionGroup
              name="fl-list"
              tag="div"
              class="mt-3 max-h-24 overflow-y-auto rounded-lg bg-muted/40 p-2"
              @click.stop
            >
              <div
                v-for="file in selectedFiles"
                :key="file"
                class="flex min-w-0 items-center gap-2 rounded-md px-2 py-1 transition-colors hover:bg-muted/60"
              >
                <SIcon icon="lucide:file" class="shrink-0 text-xs text-primary" />
                <span class="min-w-0 flex-1 truncate font-mono text-xs" :title="file">
                  {{ file }}
                </span>
                <button
                  type="button"
                  class="flex size-11 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-destructive sm:size-8"
                  aria-label="移除文件"
                  @click="removeSelectedFile(file)"
                >
                  <SIcon icon="lucide:x" class="text-xs" />
                </button>
              </div>
            </TransitionGroup>
            <div class="mt-3 flex flex-wrap gap-2" @click.stop>
              <SButton size="sm" class="min-h-11 sm:min-h-8" @click="pickFiles">
                <SIcon icon="lucide:file-plus-2" />
                继续添加
              </SButton>
              <SButton
                v-if="!isMobile"
                variant="outline"
                size="sm"
                class="min-h-11 sm:min-h-8"
                @click="pickFolder"
              >
                <SIcon icon="lucide:folder-plus" />
                添加文件夹
              </SButton>
            </div>
          </div>
          <div v-else class="flex flex-col items-center">
            <div class="flex size-8 items-center justify-center rounded-lg bg-muted text-primary">
              <SIcon icon="lucide:folder-up" class="text-base" />
            </div>
            <div class="mt-3 text-base font-semibold">
              {{ isDragging ? '松开鼠标添加文件' : '拖拽文件或文件夹到这里' }}
            </div>
            <div class="mt-1 text-xs text-muted-foreground">也可以点击下方按钮选择内容</div>
            <div class="mt-4 flex flex-wrap justify-center gap-2" @click.stop>
              <SButton size="md" class="min-h-11 sm:min-h-8" @click="pickFiles">
                <SIcon icon="lucide:file-plus-2" />
                选择文件
              </SButton>
              <SButton
                v-if="!isMobile"
                variant="outline"
                size="md"
                class="min-h-11 sm:min-h-8"
                @click="pickFolder"
              >
                <SIcon icon="lucide:folder-plus" />
                选择文件夹
              </SButton>
            </div>
          </div>
        </Transition>
      </div>
    </section>

    <!-- Send area -->
    <section class="border-t border-border pt-4" aria-labelledby="destination-title">
      <div class="flex flex-col gap-3">
        <div class="min-w-0 flex-1">
          <div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between lg:gap-6">
            <div class="flex min-w-0 flex-1 items-center justify-between gap-3">
              <div class="flex min-w-0 items-center gap-2">
                <div
                  class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-muted text-primary"
                >
                  <SIcon icon="lucide:send-to-back" class="text-base" />
                </div>
                <div class="flex min-w-0 items-center gap-2">
                  <h2 id="destination-title" class="shrink-0 text-sm font-semibold">发送到</h2>
                  <span class="min-w-0 truncate text-xs text-muted-foreground">
                    {{ selectedDevice ? selectedDevice.alias || selectedDevice.name : '选择设备' }}
                  </span>
                </div>
              </div>
              <div
                class="flex shrink-0 items-center justify-end gap-3 text-xs text-muted-foreground"
              >
                <span class="shrink-0 whitespace-nowrap">{{ onlineDevices.length }} 台在线</span>
                <button
                  type="button"
                  class="flex size-11 shrink-0 appearance-none items-center justify-center rounded-md border-0 bg-transparent p-0 text-primary outline-none transition-colors focus-visible:ring-3 focus-visible:ring-primary/30 hover:bg-primary/10 active:bg-primary/15 disabled:cursor-not-allowed disabled:bg-transparent disabled:opacity-50 sm:size-8"
                  :disabled="deviceStore.isDiscovering"
                  :aria-label="deviceStore.isDiscovering ? '正在刷新附近设备' : '刷新附近设备'"
                  :aria-busy="deviceStore.isDiscovering"
                  :title="deviceStore.isDiscovering ? '正在刷新附近设备' : '刷新附近设备'"
                  @click="refreshDevices"
                >
                  <SIcon
                    :icon="deviceStore.isDiscovering ? 'lucide:loader-circle' : 'lucide:refresh-cw'"
                    :class="deviceStore.isDiscovering ? 'animate-spin' : ''"
                  />
                </button>
              </div>
            </div>
            <SButton
              size="lg"
              class="hidden shrink-0 whitespace-nowrap lg:inline-flex lg:min-w-40 lg:w-auto"
              :disabled="!canSendFiles"
              @click="selectedDevice && sendFilesTo(selectedDevice)"
            >
              <SIcon icon="lucide:send" :class="isSendingFiles ? 'animate-pulse' : ''" />
              {{ isSendingFiles ? '发送中…' : '发送文件' }}
            </SButton>
          </div>
          <div
            v-if="deviceStore.error"
            class="mt-2 flex items-start gap-2 text-xs text-destructive"
            role="alert"
          >
            <SIcon icon="lucide:circle-alert" class="mt-1 shrink-0 text-xs" />
            <span>{{ deviceStore.error }}</span>
          </div>
          <Transition name="fl-state" mode="out-in">
            <div
              v-if="deviceStore.isDiscovering"
              key="loading"
              class="mt-3 flex gap-2 overflow-hidden"
              role="status"
              aria-live="polite"
              aria-busy="true"
            >
              <span class="sr-only">正在刷新可用设备</span>
              <SSkeleton v-for="item in 3" :key="item" class="h-11 w-32 shrink-0 sm:h-8" />
            </div>
            <TransitionGroup
              v-else-if="onlineDevices.length"
              key="devices"
              name="fl-list"
              tag="div"
              class="-mx-1 mt-3 flex flex-nowrap gap-2 overflow-x-auto px-1 pb-1 sm:mx-0 sm:flex-wrap sm:overflow-visible sm:px-0"
            >
              <button
                v-for="device in onlineDevices"
                :key="device.id"
                type="button"
                class="group inline-flex min-h-11 min-w-0 shrink-0 items-center gap-2 rounded-lg px-3 py-2 text-xs font-medium transition-all sm:min-h-8"
                :class="
                  selectedDevice?.id === device.id
                    ? 'bg-primary text-primary-foreground'
                    : 'bg-muted/60 text-foreground hover:bg-primary/10 hover:text-primary'
                "
                @click="selectedDeviceId = device.id"
              >
                <SIcon
                  :icon="device.platform === 'windows' ? 'lucide:monitor' : 'lucide:smartphone'"
                  class="shrink-0 text-xs"
                />
                <span class="max-w-40 truncate">{{ device.alias || device.name }}</span>
                <SIcon
                  icon="lucide:check"
                  class="shrink-0 text-xs transition-opacity"
                  :class="
                    selectedDevice?.id === device.id
                      ? 'opacity-100'
                      : 'opacity-0 group-hover:opacity-40'
                  "
                />
              </button>
            </TransitionGroup>
            <EmptyState
              v-else
              key="empty"
              class="mt-3"
              icon="lucide:scan-search"
              title="暂无在线设备"
              description="刷新发现，或前往附近设备手动添加"
              action-label="管理设备"
              compact
              @action="openDevices"
            />
          </Transition>
          <SButton
            size="lg"
            class="mt-3 min-h-11 w-full whitespace-nowrap lg:hidden"
            :disabled="!canSendFiles"
            @click="selectedDevice && sendFilesTo(selectedDevice)"
          >
            <SIcon icon="lucide:send" :class="isSendingFiles ? 'animate-pulse' : ''" />
            {{ isSendingFiles ? '发送中…' : '发送文件' }}
          </SButton>
        </div>
      </div>
      <Transition name="fl-state">
        <div
          v-if="sendError"
          class="mt-2 flex items-start gap-2 text-xs text-destructive"
          role="alert"
        >
          <SIcon icon="lucide:circle-alert" class="mt-1 shrink-0 text-xs" />
          <span>{{ sendError }}</span>
        </div>
      </Transition>
    </section>

    <!-- Transfer activity -->
    <section v-if="activeTasks.length || finishedTasks.length" class="border-t border-border pt-4">
      <div class="flex items-center justify-between gap-3 pb-3">
        <div class="flex min-w-0 items-center gap-2">
          <div
            class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-muted text-primary"
          >
            <SIcon icon="lucide:arrow-left-right" class="text-base" />
          </div>
          <div class="min-w-0">
            <div class="flex items-center gap-2">
              <h2 class="text-sm font-semibold leading-5">传输任务</h2>
              <StatusIndicator
                v-if="activeTasks.length"
                :label="`${activeTasks.length} 个进行中`"
                tone="primary"
                pulse
              />
            </div>
            <p class="mt-1 text-xs text-muted-foreground">
              {{ activeTasks.length ? '实时同步文件收发状态' : '本次传输已全部处理完成' }}
            </p>
          </div>
        </div>
        <button
          class="flex shrink-0 items-center gap-1 text-xs font-medium text-muted-foreground transition-colors hover:text-primary"
          type="button"
          @click="openHistory"
        >
          <span class="hidden sm:inline">查看传输记录</span>
          <span class="sm:hidden">查看记录</span>
          <SIcon icon="lucide:arrow-up-right" class="text-sm" />
        </button>
      </div>

      <TransitionGroup
        v-if="activeTasks.length"
        name="fl-list"
        tag="div"
        class="mt-3 divide-y divide-border overflow-hidden rounded-lg border border-border bg-card"
      >
        <div v-for="task in activeTasks" :key="task.id" class="px-3 py-3 sm:px-4">
          <div class="flex items-start gap-3">
            <div
              class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-muted text-primary"
            >
              <SIcon
                :icon="task.direction === 'receive' ? 'lucide:download' : 'lucide:file-up'"
                class="text-sm"
              />
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <div class="truncate text-xs font-semibold leading-5" :title="task.fileName">
                    {{ task.fileName }}
                  </div>
                  <div
                    class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground"
                  >
                    <span>
                      {{ task.direction === 'receive' ? '接收自' : '发送至' }} {{ task.targetIp }}
                    </span>
                    <span v-if="task.speed" class="text-border">·</span>
                    <span v-if="task.speed">{{ formatSpeed(task.speed) }}</span>
                  </div>
                </div>
                <StatusIndicator
                  class="shrink-0"
                  :label="statusLabel(task)"
                  tone="primary"
                  :pulse="task.status === 'transferring'"
                />
              </div>

              <div class="mt-2 flex items-center gap-2">
                <div class="h-1.5 min-w-0 flex-1 overflow-hidden rounded-full bg-muted">
                  <div
                    class="h-full rounded-full bg-primary transition-all duration-500"
                    :style="{ width: `${task.progress}%` }"
                  />
                </div>
                <span class="shrink-0 text-xs font-semibold tabular-nums text-foreground">
                  {{ task.progress.toFixed(0) }}%
                </span>
                <SButton
                  variant="ghost"
                  color="destructive"
                  size="sm"
                  shape="square"
                  class="size-11 shrink-0 sm:size-8"
                  aria-label="取消传输"
                  title="取消传输"
                  @click="cancelTransfer(task)"
                >
                  <SIcon icon="lucide:circle-x" />
                </SButton>
              </div>
              <div
                class="mt-2 flex items-center justify-between gap-3 text-xs text-muted-foreground"
              >
                <span v-if="task.error" class="min-w-0 truncate text-destructive">
                  {{ task.error }}
                </span>
                <span v-else>{{ task.status === 'pending' ? '等待开始' : '正在传输文件' }}</span>
                <span class="shrink-0 tabular-nums">
                  {{ formatBytes(task.transferred) }}
                  <span v-if="task.total">/ {{ formatBytes(task.total) }}</span>
                </span>
              </div>
            </div>
          </div>
        </div>
      </TransitionGroup>

      <div v-else class="mt-3 flex items-center gap-2 rounded-lg bg-muted/50 px-3 py-3 sm:px-4">
        <div class="flex size-8 shrink-0 items-center justify-center rounded-lg text-success">
          <SIcon icon="lucide:check" class="text-sm" />
        </div>
        <p class="min-w-0 flex-1 text-xs text-muted-foreground">
          已完成 {{ completedCount }} 个传输任务
          <span v-if="failedCount">，{{ failedCount }} 个任务失败</span>
          ；完整文件记录已收纳至传输记录。
        </p>
        <button
          class="shrink-0 text-xs font-medium text-primary hover:underline"
          type="button"
          @click="openHistory"
        >
          去查看
        </button>
      </div>
    </section>

    <section v-else class="border-t border-border pt-4">
      <div class="flex items-center gap-2 px-3 text-xs text-muted-foreground sm:px-4">
        <div
          class="flex size-8 shrink-0 items-center justify-center rounded-lg text-muted-foreground"
        >
          <SIcon icon="lucide:arrow-left-right" class="text-sm" />
        </div>
        <span>暂无进行中的任务，选择文件发送，或等待其他设备发送到本机</span>
      </div>
    </section>
  </div>
</template>
