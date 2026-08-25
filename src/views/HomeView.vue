<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { SButton } from '@/ui/components/button'
import { SCard } from '@/ui/components/card'
import { SIcon } from '@/ui/components/icon'
import { SBadge } from '@/ui/components/badge'
import { SSeparator } from '@/ui/components/separator'
import { isTauri } from '@/utils/tauri'
import { useDeviceStore } from '@/stores/device'
import { useTransferStore } from '@/stores/transfer'

const deviceStore = useDeviceStore()
const transferStore = useTransferStore()

const isDragging = ref(false)
const selectedFiles = ref<string[]>([])
const manualIp = ref('')
const showManual = ref(false)

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
        <p class="text-sm text-muted-foreground mt-1">拖拽文件到此处，或选择设备直接发送</p>
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
          class="hover:border-primary/30 hover:shadow-sm cursor-pointer transition-all group p-4!"
        >
          <div class="flex items-start justify-between">
            <div class="size-10 rounded-xl bg-muted flex items-center justify-center">
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
            <div class="size-2 rounded-full bg-success mt-1 shrink-0" />
          </div>
          <div class="mt-3 font-medium text-sm truncate">{{ device.name }}</div>
          <div class="text-xs text-muted-foreground font-mono truncate">
            {{ device.ip }}:{{ device.port }}
          </div>
          <div class="text-xs text-muted-foreground truncate">{{ device.platform }}</div>
          <div class="mt-3">
            <SButton size="sm" class="w-full" @click="handleSend(device.ip, device.port)">
              <SIcon icon="lucide:send" />
              发送
            </SButton>
          </div>
        </SCard>

        <!-- Add manual IP -->
        <SCard
          class="border-dashed! bg-muted/20 hover:bg-card hover:border-border transition-colors min-h-[168px] flex flex-col items-center justify-center text-center cursor-pointer p-4!"
          @click="showManual = !showManual"
        >
          <div
            class="size-8 rounded-full border flex items-center justify-center text-muted-foreground"
          >
            <SIcon icon="lucide:plus" />
          </div>
          <div class="text-sm font-medium mt-2">手动添加设备</div>
          <div class="text-xs text-muted-foreground">输入 IP 地址</div>
          <div v-if="showManual" class="mt-3 w-full flex gap-2" @click.stop>
            <input
              v-model="manualIp"
              placeholder="192.168.1.100"
              class="flex-1 border rounded px-2 py-1 text-xs bg-background"
            />
            <SButton size="sm" @click="handleSend(manualIp)">连接</SButton>
          </div>
        </SCard>
      </div>
    </div>

    <SSeparator />

    <!-- Transfers -->
    <SCard v-if="transferStore.tasks.length > 0">
      <template #header>
        <div class="flex items-center justify-between w-full">
          <span class="font-medium text-sm flex items-center gap-2">
            <SIcon icon="lucide:arrow-left-right" class="text-muted-foreground" />
            传输任务
          </span>
          <SBadge color="secondary" size="sm">{{ transferStore.tasks.length }}</SBadge>
        </div>
      </template>
      <div class="space-y-3">
        <div
          v-for="task in transferStore.tasks"
          :key="task.id"
          class="flex items-center gap-4 p-3 rounded-lg border bg-card"
        >
          <div class="size-10 rounded-lg bg-primary/10 flex items-center justify-center shrink-0">
            <SIcon icon="lucide:file-text" class="text-primary" />
          </div>
          <div class="flex-1 min-w-0">
            <div class="text-sm font-medium truncate flex items-center gap-2">
              {{ task.fileName }}
              <SBadge
                :color="
                  task.status === 'completed'
                    ? 'success'
                    : task.status === 'failed'
                      ? 'destructive'
                      : 'secondary'
                "
                size="sm"
              >
                {{
                  task.status === 'completed'
                    ? '已完成'
                    : task.status === 'failed'
                      ? '失败'
                      : task.status === 'transferring'
                        ? '传输中'
                        : '等待中'
                }}
              </SBadge>
            </div>
            <div class="text-xs text-muted-foreground flex items-center gap-1.5 mt-1">
              <span>{{ task.targetIp }}</span>
              <span v-if="task.speed">· {{ formatSpeed(task.speed) }}</span>
            </div>
            <div v-if="task.error" class="text-xs text-destructive mt-1 truncate">
              {{ task.error }}
            </div>
            <div class="mt-2 h-1.5 rounded-full bg-muted overflow-hidden">
              <div
                class="h-full bg-primary rounded-full transition-all"
                :style="{ width: `${task.progress}%` }"
              ></div>
            </div>
          </div>
          <div class="text-right shrink-0">
            <div class="text-sm font-medium">{{ task.progress.toFixed(0) }}%</div>
            <div class="text-xs text-muted-foreground">{{ task.transferred }}/{{ task.total }}</div>
          </div>
        </div>
      </div>
    </SCard>
    <SCard v-else>
      <template #header>
        <div class="flex items-center justify-between w-full">
          <span class="font-medium text-sm flex items-center gap-2">
            <SIcon icon="lucide:arrow-left-right" class="text-muted-foreground" />
            正在传输
          </span>
          <SBadge color="secondary" size="sm">0</SBadge>
        </div>
      </template>
      <div class="p-4 text-center text-sm text-muted-foreground">
        暂无传输任务，选择文件并发送至设备
      </div>
    </SCard>
  </div>
</template>
