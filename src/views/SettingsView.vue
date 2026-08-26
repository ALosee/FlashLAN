<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { SCard } from '@/ui/components/card'
import { SButton } from '@/ui/components/button'
import { SInput } from '@/ui/components/input'
import { SSwitch } from '@/ui/components/switch'
import { SIcon } from '@/ui/components/icon'
import { SSeparator } from '@/ui/components/separator'
import { useDeviceStore } from '@/stores/device'
import { useTransferStore } from '@/stores/transfer'
import { isTauri } from '@/utils/tauri'

const deviceStore = useDeviceStore()
const transferStore = useTransferStore()
const savePath = ref('')
const deviceName = ref('')
const isLoading = ref(true)
const isSavingName = ref(false)
const isChoosingPath = ref(false)
const settingsMessage = ref('')
const settingsError = ref('')

interface SettingsPayload {
  device_name: string
  save_path: string
}

const browserSettingsStorageKey = 'flashlan.settings'

const autoReceive = computed<boolean>({
  get: () => transferStore.autoReceiveEnabled,
  set: value => {
    void transferStore.setAutoReceive(value).catch(error => {
      console.error('[FlashLAN] update auto receive failed', error)
    })
  },
})

const displayedSavePath = computed(() => {
  if (savePath.value.length <= 72) return savePath.value
  const parts = savePath.value.split(/[\\/]/).filter(Boolean)
  return parts.length > 2 ? `…/${parts.slice(-2).join('/')}` : savePath.value
})

function clearFeedback() {
  settingsMessage.value = ''
  settingsError.value = ''
}

function loadBrowserSettings(): SettingsPayload {
  const fallback = {
    device_name: deviceStore.localDevice?.name || 'Browser Preview',
    save_path: 'Download/FlashLAN',
  }
  if (typeof localStorage === 'undefined') return fallback

  try {
    const stored = JSON.parse(
      localStorage.getItem(browserSettingsStorageKey) || 'null',
    ) as Partial<SettingsPayload> | null
    return {
      device_name: stored?.device_name || fallback.device_name,
      save_path: stored?.save_path || fallback.save_path,
    }
  } catch {
    return fallback
  }
}

async function loadSettings() {
  isLoading.value = true
  clearFeedback()
  try {
    await deviceStore.fetchLocal()
    const settings = isTauri()
      ? await invoke<SettingsPayload>('get_settings')
      : loadBrowserSettings()
    deviceName.value = settings.device_name || deviceStore.localDevice?.name || ''
    savePath.value = settings.save_path || 'Download/FlashLAN'
  } catch (error) {
    settingsError.value = String(error).replace(/^Error:\s*/, '') || '设置加载失败'
    if (deviceStore.localDevice) {
      deviceName.value = deviceStore.localDevice.name
    }
    savePath.value = 'Download/FlashLAN'
  } finally {
    isLoading.value = false
  }
}

async function saveDeviceName() {
  const name = deviceName.value.trim()
  clearFeedback()
  if (!name) {
    settingsError.value = '设备名称不能为空'
    return
  }

  isSavingName.value = true
  try {
    if (isTauri()) {
      await invoke('set_device_name', { name })
    } else if (typeof localStorage !== 'undefined') {
      const settings = loadBrowserSettings()
      localStorage.setItem(
        browserSettingsStorageKey,
        JSON.stringify({ ...settings, device_name: name }),
      )
    }
    deviceName.value = name
    if (deviceStore.localDevice) {
      deviceStore.localDevice.name = name
    }
    settingsMessage.value = '设备名称已保存'
  } catch (error) {
    settingsError.value = String(error).replace(/^Error:\s*/, '') || '设备名称保存失败'
  } finally {
    isSavingName.value = false
  }
}

async function chooseSavePath() {
  if (!isTauri()) {
    settingsError.value = '浏览器预览无法访问本机目录，请在桌面应用中选择保存路径'
    return
  }

  clearFeedback()
  isChoosingPath.value = true
  try {
    const selected = await open({ directory: true, multiple: false })
    if (typeof selected !== 'string' || !selected) return

    await invoke('set_save_path', { path: selected })
    savePath.value = selected
    settingsMessage.value = '保存路径已更新'
  } catch (error) {
    settingsError.value = String(error).replace(/^Error:\s*/, '') || '保存路径更新失败'
  } finally {
    isChoosingPath.value = false
  }
}

onMounted(loadSettings)
</script>

<template>
  <div class="p-4 md:p-6 max-w-3xl mx-auto w-full space-y-4 md:space-y-6">
    <div>
      <h1 class="text-xl font-bold tracking-tight">设置</h1>
      <p class="text-sm text-muted-foreground mt-1">管理设备与传输偏好</p>
    </div>

    <SCard :class="isLoading ? 'opacity-70' : ''">
      <div class="divide-y">
        <div
          class="py-3 md:py-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3 first:pt-0 last:pb-0"
        >
          <div class="flex items-center gap-3">
            <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
              <SIcon icon="lucide:monitor" />
            </div>
            <div>
              <div class="text-sm font-medium">设备名称</div>
              <div class="text-xs text-muted-foreground">局域网内显示的名称</div>
            </div>
          </div>
          <div class="flex w-full items-center gap-2 sm:w-auto">
            <SInput
              v-model="deviceName"
              class="w-full sm:w-56"
              placeholder="输入设备名称"
              :disabled="isLoading || isSavingName"
              @keyup.enter="saveDeviceName"
            />
            <SButton
              size="sm"
              class="shrink-0"
              :disabled="isLoading || isSavingName"
              @click="saveDeviceName"
            >
              {{ isSavingName ? '保存中...' : '保存' }}
            </SButton>
          </div>
        </div>

        <div class="py-3 md:py-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div class="flex items-center gap-3">
            <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
              <SIcon icon="lucide:folder" />
            </div>
            <div>
              <div class="text-sm font-medium">保存路径</div>
              <div class="text-xs text-muted-foreground truncate max-w-[360px]" :title="savePath">
                {{ displayedSavePath || '加载中...' }}
              </div>
            </div>
          </div>
          <SButton
            variant="outline"
            size="sm"
            class="shrink-0"
            :disabled="isLoading || isChoosingPath"
            @click="chooseSavePath"
          >
            <SIcon icon="lucide:folder-open" />
            {{ isChoosingPath ? '选择中...' : '选择' }}
          </SButton>
        </div>

        <div class="py-3 md:py-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div class="flex items-center gap-3">
            <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
              <SIcon icon="lucide:shield-check" />
            </div>
            <div>
              <div class="text-sm font-medium">可信设备自动接收</div>
              <div class="text-xs text-muted-foreground">开启后可信设备发送文件将自动接收</div>
            </div>
          </div>
          <SSwitch v-model="autoReceive" />
        </div>

        <div class="py-3 md:py-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div class="flex items-center gap-3">
            <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
              <SIcon icon="lucide:plug" />
            </div>
            <div>
              <div class="text-sm font-medium">端口</div>
              <div class="text-xs text-muted-foreground">发现 mDNS / 传输 17321</div>
            </div>
          </div>
          <span class="text-xs font-mono bg-muted px-2.5 py-1 rounded-md">17321</span>
        </div>

        <div class="py-3 md:py-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div class="flex items-center gap-3">
            <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
              <SIcon icon="lucide:info" />
            </div>
            <div>
              <div class="text-sm font-medium">本机信息</div>
              <div class="text-xs text-muted-foreground font-mono">
                {{ deviceStore.localDevice?.id || '-' }}
              </div>
            </div>
          </div>
          <span class="text-xs bg-muted px-2 py-1 rounded">
            {{ deviceStore.localDevice?.platform || '-' }}
          </span>
        </div>
      </div>
    </SCard>

    <div
      v-if="settingsMessage || settingsError"
      class="flex items-center gap-2 text-xs"
      :class="settingsError ? 'text-destructive' : 'text-success'"
    >
      <SIcon :icon="settingsError ? 'lucide:circle-alert' : 'lucide:check-circle-2'" />
      {{ settingsError || settingsMessage }}
    </div>

    <SSeparator />

    <div class="text-xs text-muted-foreground flex items-center gap-2">
      <SIcon icon="lucide:info" class="text-xs" />
      FlashLAN v0.1.0 · Tauri 2 · Vue 3 · SoybeanUI · UnoCSS · mDNS
    </div>
  </div>
</template>
