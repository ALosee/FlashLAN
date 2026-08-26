<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { open } from '@tauri-apps/plugin-dialog'
import { SButton } from '@/ui/components/button'
import { SCard } from '@/ui/components/card'
import { SDialog } from '@/ui/components/dialog'
import { SIcon } from '@/ui/components/icon'
import { SInput } from '@/ui/components/input'
import { SBadge } from '@/ui/components/badge'
import { isTauri } from '@/utils/tauri'
import { useDeviceStore } from '@/stores/device'
import { type TransferTask, useTransferStore } from '@/stores/transfer'

const deviceStore = useDeviceStore()
const transferStore = useTransferStore()
const router = useRouter()

const isDragging = ref(false)
const selectedFiles = ref<string[]>([])
const manualIp = ref('')
const manualPort = ref('17321')
const showAddDevice = ref(false)
const isTestingConnection = ref(false)
const connectionState = ref<'idle' | 'testing' | 'success' | 'error'>('idle')
const connectionMessage = ref('')
const testedEndpoint = ref('')

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

const manualEndpoint = computed(() => `${manualIp.value.trim()}:${manualPort.value.trim()}`)
const canAddManualDevice = computed(
  () => connectionState.value === 'success' && testedEndpoint.value === manualEndpoint.value,
)

const ipv4Pattern = /^(25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)){3}$/

function clearConnectionState() {
  connectionState.value = 'idle'
  connectionMessage.value = ''
  testedEndpoint.value = ''
}

watch([manualIp, manualPort], clearConnectionState)

function openAddDevice() {
  manualIp.value = ''
  manualPort.value = '17321'
  clearConnectionState()
  showAddDevice.value = true
}

function closeAddDevice() {
  if (isTestingConnection.value) return
  showAddDevice.value = false
}

function getManualDeviceInput() {
  const ip = manualIp.value.trim()
  const port = Number(manualPort.value)

  if (!ip) {
    connectionState.value = 'error'
    connectionMessage.value = '请输入设备 IP 地址'
    return null
  }
  if (!ipv4Pattern.test(ip)) {
    connectionState.value = 'error'
    connectionMessage.value = '请输入有效的 IPv4 地址'
    return null
  }
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    connectionState.value = 'error'
    connectionMessage.value = '端口号必须在 1 到 65535 之间'
    return null
  }

  return { ip, port }
}

function getErrorMessage(error: unknown) {
  const message = String(error)
    .replace(/^Error:\s*/, '')
    .trim()
  return message || '连接失败，请确认设备地址和端口正确'
}

async function testManualConnection() {
  const input = getManualDeviceInput()
  if (!input) return

  const endpoint = `${input.ip}:${input.port}`
  isTestingConnection.value = true
  connectionState.value = 'testing'
  connectionMessage.value = `正在测试 ${endpoint}...`

  try {
    await deviceStore.testConnection(input.ip, input.port)
    if (manualEndpoint.value !== endpoint) {
      connectionState.value = 'idle'
      connectionMessage.value = '地址已变化，请重新测试连接'
      return
    }
    connectionState.value = 'success'
    testedEndpoint.value = endpoint
    connectionMessage.value = `连接成功，可以添加设备`
  } catch (error) {
    connectionState.value = 'error'
    connectionMessage.value = getErrorMessage(error)
  } finally {
    isTestingConnection.value = false
  }
}

async function addManualDevice() {
  const input = getManualDeviceInput()
  if (!input) return

  if (!canAddManualDevice.value) {
    await testManualConnection()
  }
  if (!canAddManualDevice.value) return

  deviceStore.addManualDevice(input.ip, input.port)
  showAddDevice.value = false
}

function onDragOver(e: DragEvent) {
  e.preventDefault()
  isDragging.value = true
}
function onDragLeave() {
  isDragging.value = false
}
function onDrop(e: DragEvent) {
  e.preventDefault()
  isDragging.value = false
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
        selectedFiles.value = Array.from(files).map(f => f.name)
      }
    })
    input.click()
    return
  }
  try {
    const files = await open({ multiple: true, directory: false })
    if (files) {
      const list = Array.isArray(files) ? files : [files]
      selectedFiles.value = list
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
        selectedFiles.value = [first.webkitRelativePath?.split('/')[0] || first.name]
      }
    })
    input.click()
    return
  }
  try {
    const folder = await open({ directory: true, multiple: false })
    if (folder && typeof folder === 'string') {
      selectedFiles.value = [folder]
    }
  } catch (e) {
    console.warn('open folder failed', e)
  }
}

async function handleSend(deviceIp: string, devicePort?: number) {
  if (selectedFiles.value.length === 0) {
    if (!isTauri()) {
      selectedFiles.value = ['browser-file-mock.txt']
    } else {
      try {
        const files = await open({ multiple: true, directory: false })
        if (!files) return
        const list = Array.isArray(files) ? files : [files]
        selectedFiles.value = list
      } catch {
        return
      }
    }
  }
  for (const file of selectedFiles.value) {
    try {
      await transferStore.sendFile(file, deviceIp, devicePort)
    } catch (e) {
      console.error('send failed', e)
    }
  }
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

function platformLabel(platform: string) {
  if (platform === 'macos') return 'macOS'
  if (platform === 'windows') return 'Windows'
  if (platform === 'android') return 'Android'
  if (platform === 'ios') return 'iPhone / iPad'
  if (platform === 'manual') return '手动添加'
  return platform
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

onMounted(async () => {
  await deviceStore.fetchLocal()
  await deviceStore.discover()
  await transferStore.ensureListener()
})
</script>

<template>
  <div class="p-4 md:p-6 space-y-4 md:space-y-6 max-w-5xl mx-auto w-full">
    <!-- Header -->
    <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
      <div class="flex-1 min-w-0">
        <h1 class="text-xl font-bold tracking-tight">快速传文件</h1>
        <p class="text-sm text-muted-foreground mt-1">
          拖拽文件到此处，或选择设备直接发送；接收文件会自动显示在这里
        </p>
      </div>
      <div
        class="flex flex-wrap items-center gap-2 sm:shrink-0 sm:justify-end bg-muted/50 sm:bg-transparent px-3 py-2 sm:p-0 rounded-lg"
      >
        <span class="text-xs text-muted-foreground">本机可被发现</span>
        <div class="size-2 rounded-full bg-success shrink-0" />
        <span class="text-xs font-medium text-success">在线</span>
        <span
          v-if="deviceStore.localDevice"
          class="text-xs font-mono bg-muted px-2 py-1 rounded truncate max-w-[160px] sm:max-w-none"
        >
          {{ deviceStore.localDevice.ip }}:{{ deviceStore.localDevice.port }}
        </span>
      </div>
    </div>

    <!-- Selected files -->
    <SCard v-if="selectedFiles.length > 0">
      <template #header>
        <span class="text-sm font-medium flex items-center gap-2">
          <SIcon icon="lucide:files" />
          已选 {{ selectedFiles.length }} 个文件
        </span>
      </template>
      <div class="space-y-1">
        <div
          v-for="f in selectedFiles"
          :key="f"
          class="text-xs font-mono bg-muted px-3 py-2 rounded truncate"
        >
          {{ f }}
        </div>
        <SButton variant="ghost" size="sm" class="mt-2" @click="selectedFiles = []">
          <SIcon icon="lucide:x" />
          清空
        </SButton>
      </div>
    </SCard>

    <!-- Drop zone -->
    <SCard class="border-dashed! border-2!">
      <div
        class="p-6 md:p-10 text-center transition-colors cursor-pointer rounded-xl"
        :class="isDragging ? 'bg-primary/5 border-primary' : 'bg-card hover:bg-muted/30'"
        @dragover="onDragOver"
        @dragleave="onDragLeave"
        @drop="onDrop"
        @click="pickFiles"
      >
        <div class="size-14 rounded-2xl bg-primary/10 flex items-center justify-center mx-auto">
          <SIcon icon="lucide:folder-open" class="text-2xl text-primary" />
        </div>
        <div class="mt-4 font-medium">拖拽文件或文件夹到此处</div>
        <div class="text-sm text-muted-foreground mt-1">支持多文件、文件夹，单次可传任意大小</div>
        <div class="mt-5 flex items-center justify-center gap-3" @click.stop>
          <SButton @click="pickFiles">
            <SIcon icon="lucide:file-up" />
            选择文件
          </SButton>
          <SButton variant="outline" @click="pickFolder">
            <SIcon icon="lucide:folder" />
            选择文件夹
          </SButton>
        </div>
        <div class="text-xs text-muted-foreground mt-3 flex items-center justify-center gap-1.5">
          <SIcon icon="lucide:clipboard" class="text-xs" />
          或直接粘贴剪贴板内容 Ctrl+V
        </div>
      </div>
    </SCard>

    <!-- Devices grid -->
    <div>
      <div class="flex items-center justify-between">
        <h2 class="font-semibold flex items-center gap-2">
          附近设备
          <SBadge color="secondary" size="sm">{{ deviceStore.devices.length }}</SBadge>
          <span v-if="deviceStore.isDiscovering" class="text-xs text-muted-foreground">
            扫描中...
          </span>
          <span v-if="deviceStore.error" class="text-xs text-destructive">
            {{ deviceStore.error }}
          </span>
        </h2>
        <div class="flex items-center gap-1">
          <SButton
            variant="ghost"
            size="sm"
            :disabled="deviceStore.isDiscovering"
            @click="deviceStore.discover()"
          >
            <SIcon
              icon="lucide:refresh-cw"
              :class="deviceStore.isDiscovering ? 'animate-spin' : ''"
            />
            刷新
          </SButton>
          <SButton size="sm" @click="openAddDevice">
            <SIcon icon="lucide:plus" />
            添加设备
          </SButton>
        </div>
      </div>

      <div
        v-if="deviceStore.devices.length === 0 && !deviceStore.isDiscovering"
        class="mt-3 rounded-xl border border-dashed p-8 text-center text-sm text-muted-foreground"
      >
        未发现设备，请确保两台设备在同一局域网且已启动 FlashLAN
      </div>

      <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 mt-3">
        <SCard
          v-for="device in deviceStore.devices"
          :key="device.id"
          class="group relative overflow-hidden border-border/80 dark:border-border/10 bg-card/95 p-3! transition-all"
        >
          <div class="flex items-center justify-between gap-3">
            <div class="flex min-w-0 items-center gap-3">
              <div
                class="flex size-11 shrink-0 items-center justify-center rounded-2xl bg-gradient-to-br from-primary/15 via-primary/8 to-muted text-primary ring-1 ring-primary/10"
              >
                <SIcon
                  :icon="
                    device.platform === 'windows'
                      ? 'lucide:monitor'
                      : device.platform === 'macos'
                        ? 'lucide:laptop'
                        : 'lucide:smartphone'
                  "
                  class="text-lg"
                />
              </div>
              <div class="min-w-0">
                <div class="truncate text-sm font-semibold leading-5">{{ device.name }}</div>
                <div class="mt-0.5 truncate font-mono text-[11px] text-muted-foreground">
                  {{ device.ip }}:{{ device.port }}
                </div>
              </div>
            </div>
            <span
              class="inline-flex shrink-0 items-center gap-1.5 rounded-full bg-success/10 px-2 py-1 text-[10px] font-medium text-success"
            >
              <span class="size-1.5 rounded-full bg-success" />
              在线
            </span>
          </div>

          <div class="mt-3 flex items-center justify-between gap-3">
            <span class="rounded-md bg-muted/70 px-2 py-1 text-[11px] text-muted-foreground">
              {{ platformLabel(device.platform) }}
            </span>
            <span class="text-[11px] text-muted-foreground">可发送文件</span>
          </div>

          <div class="mt-3">
            <SButton size="sm" class="h-9 w-full" @click="handleSend(device.ip, device.port)">
              <SIcon icon="lucide:send" />
              发送文件
            </SButton>
          </div>
        </SCard>
      </div>
    </div>

    <SDialog
      v-model:open="showAddDevice"
      title="添加设备"
      description="输入设备地址并测试连接，确认可用后再添加。"
      size="sm"
      :show-fullscreen="false"
      :show-confirm="false"
      :show-cancel="false"
    >
      <form class="space-y-4" @submit.prevent="testManualConnection">
        <div class="grid grid-cols-[minmax(0,1fr)_7rem] gap-3">
          <div class="space-y-1.5">
            <label for="manual-device-ip" class="text-sm font-medium">IP 地址</label>
            <SInput
              id="manual-device-ip"
              v-model="manualIp"
              autofocus
              placeholder="192.168.1.100"
              autocomplete="off"
            />
          </div>
          <div class="space-y-1.5">
            <label for="manual-device-port" class="text-sm font-medium">端口</label>
            <SInput
              id="manual-device-port"
              v-model="manualPort"
              type="number"
              min="1"
              max="65535"
              inputmode="numeric"
              placeholder="17321"
            />
          </div>
        </div>

        <p class="text-xs text-muted-foreground">
          默认端口为 17321，请确认目标设备已启动 FlashLAN。
        </p>

        <div
          v-if="connectionState !== 'idle'"
          class="flex items-start gap-2 rounded-lg px-3 py-2 text-xs"
          :class="
            connectionState === 'success'
              ? 'bg-success/10 text-success'
              : connectionState === 'error'
                ? 'bg-destructive/10 text-destructive'
                : 'bg-muted text-muted-foreground'
          "
          role="status"
        >
          <SIcon
            :icon="
              connectionState === 'success'
                ? 'lucide:circle-check'
                : connectionState === 'error'
                  ? 'lucide:circle-alert'
                  : 'lucide:loader-circle'
            "
            :class="connectionState === 'testing' ? 'animate-spin' : ''"
            class="text-sm shrink-0 mt-0.5"
          />
          <span>{{ connectionMessage }}</span>
        </div>

        <SButton type="submit" variant="outline" class="w-full" :disabled="isTestingConnection">
          <SIcon icon="lucide:plug" :class="isTestingConnection ? 'animate-pulse' : ''" />
          {{ isTestingConnection ? '测试连接中...' : '测试连接' }}
        </SButton>
      </form>

      <template #footer>
        <SButton variant="ghost" :disabled="isTestingConnection" @click="closeAddDevice">
          取消
        </SButton>
        <SButton :disabled="!canAddManualDevice || isTestingConnection" @click="addManualDevice">
          <SIcon icon="lucide:plus" />
          添加设备
        </SButton>
      </template>
    </SDialog>

    <!-- Transfer activity -->
    <section
      v-if="activeTasks.length || finishedTasks.length"
      class="relative overflow-hidden rounded-[1.4rem] border border-border/80 dark:border-border/10"
    >
      <div
        class="relative min-h-20 border-b border-border/70 bg-gradient-to-br from-primary/8 via-card to-card px-4 py-4 sm:px-5"
      >
        <div class="flex min-w-0 items-center gap-3 pr-24 sm:pr-36">
          <div
            class="relative flex size-11 shrink-0 items-center justify-center rounded-2xl bg-primary/12 text-primary ring-1 ring-primary/10"
          >
            <span
              v-if="activeTasks.length"
              class="absolute inset-0 animate-ping rounded-2xl bg-primary/10"
            />
            <SIcon icon="lucide:arrow-left-right" class="relative text-lg" />
          </div>
          <div class="min-w-0">
            <div class="flex items-center gap-2">
              <h2 class="text-sm font-semibold leading-5">传输任务</h2>
              <span
                v-if="activeTasks.length"
                class="inline-flex items-center gap-1.5 rounded-full bg-primary/10 px-2 py-1 text-[10px] font-medium text-primary"
              >
                <span class="size-1.5 rounded-full bg-primary" />
                {{ activeTasks.length }} 个进行中
              </span>
            </div>
            <p class="mt-0.5 text-[11px] text-muted-foreground">
              {{ activeTasks.length ? '实时同步文件收发状态' : '本次传输已全部处理完成' }}
            </p>
          </div>
        </div>
        <button
          class="absolute right-4 top-1/2 flex -translate-y-1/2 shrink-0 items-center gap-1 text-[11px] font-medium text-muted-foreground transition-colors hover:text-primary sm:right-5 sm:text-xs"
          type="button"
          @click="openHistory"
        >
          <span class="hidden sm:inline">查看传输记录</span>
          <span class="sm:hidden">查看记录</span>
          <SIcon icon="lucide:arrow-up-right" class="text-sm" />
        </button>
      </div>

      <div v-if="activeTasks.length" class="relative space-y-2.5 bg-muted/15 p-3 sm:p-4">
        <div
          v-for="task in activeTasks"
          :key="task.id"
          class="group rounded-2xl border border-border/70 bg-card/90 p-3.5 transition-all hover:border-primary/25 hover:shadow-sm sm:p-4"
        >
          <div class="flex items-start gap-3">
            <div
              class="flex size-10 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary"
            >
              <SIcon
                :icon="task.direction === 'receive' ? 'lucide:download' : 'lucide:file-up'"
                class="text-lg"
              />
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <div class="truncate text-sm font-semibold leading-5" :title="task.fileName">
                    {{ task.fileName }}
                  </div>
                  <div
                    class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-muted-foreground"
                  >
                    <span>
                      {{ task.direction === 'receive' ? '接收自' : '发送至' }} {{ task.targetIp }}
                    </span>
                    <span v-if="task.speed" class="text-border">·</span>
                    <span v-if="task.speed">{{ formatSpeed(task.speed) }}</span>
                  </div>
                </div>
                <span
                  class="inline-flex shrink-0 items-center gap-1 rounded-full bg-primary/10 px-2 py-1 text-[10px] font-medium text-primary"
                >
                  <SIcon
                    :icon="task.status === 'pending' ? 'lucide:clock-3' : 'lucide:loader-circle'"
                    class="text-[11px]"
                    :class="task.status === 'transferring' ? 'animate-spin' : ''"
                  />
                  {{ statusLabel(task) }}
                </span>
              </div>

              <div class="mt-3 flex items-center gap-3">
                <div class="h-2 min-w-0 flex-1 overflow-hidden rounded-full bg-muted">
                  <div
                    class="h-full rounded-full bg-gradient-to-r from-primary to-primary/60 transition-all duration-500"
                    :style="{ width: `${task.progress}%` }"
                  />
                </div>
                <span class="shrink-0 text-xs font-semibold tabular-nums text-foreground">
                  {{ task.progress.toFixed(0) }}%
                </span>
              </div>
              <div
                class="mt-2 flex items-center justify-between gap-3 text-[11px] text-muted-foreground"
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
      </div>

      <div v-else class="relative flex items-center gap-3 bg-muted/15 px-4 py-4 sm:px-5">
        <div
          class="flex size-9 shrink-0 items-center justify-center rounded-xl bg-success/10 text-success"
        >
          <SIcon icon="lucide:check" />
        </div>
        <p class="min-w-0 flex-1 text-xs text-muted-foreground">
          已完成 {{ completedCount }} 个传输任务
          <span v-if="failedCount">，{{ failedCount }} 个任务失败</span>
          ；完整文件记录已收纳至传输记录。
        </p>
        <button
          class="hidden shrink-0 text-xs font-medium text-primary hover:underline sm:block"
          type="button"
          @click="openHistory"
        >
          去查看
        </button>
      </div>
    </section>

    <SCard v-else>
      <div class="flex items-center gap-3 p-1 text-sm text-muted-foreground">
        <div class="flex size-9 shrink-0 items-center justify-center rounded-xl bg-muted">
          <SIcon icon="lucide:arrow-left-right" />
        </div>
        <span>暂无进行中的任务，选择文件发送，或等待其他设备发送到本机</span>
      </div>
    </SCard>
  </div>
</template>
