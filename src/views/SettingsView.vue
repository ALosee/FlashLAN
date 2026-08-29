<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { useTheme } from '@soybeanjs/ui'
import type { ThemeModePreference } from '@soybeanjs/theme'
import { version as appVersion } from '../../package.json'
import { SCard } from '@/ui/components/card'
import { SButton } from '@/ui/components/button'
import { SInput } from '@/ui/components/input'
import { SSwitch } from '@/ui/components/switch'
import { SIcon } from '@/ui/components/icon'
import { SSeparator } from '@/ui/components/separator'
import { PageHeader, SettingsRow } from '@/ui/patterns'
import { useDeviceStore } from '@/stores/device'
import { useTransferStore } from '@/stores/transfer'
import { isMobilePlatform, isTauri } from '@/utils/tauri'

const deviceStore = useDeviceStore()
const transferStore = useTransferStore()
const savePath = ref('')
const deviceName = ref('')
const savedDeviceName = ref('')
const isLoading = ref(true)
const isSavingName = ref(false)
const isChoosingPath = ref(false)
const settingsMessage = ref('')
const settingsError = ref('')
const isMobile = isMobilePlatform()
const theme = useTheme()
const themeMode = computed<ThemeModePreference>(() => theme?.mode.value ?? 'light')
const canSaveDeviceName = computed(() => {
  const name = deviceName.value.trim()
  return Boolean(name) && name !== savedDeviceName.value
})

const themeModeOptions: Array<{
  value: ThemeModePreference
  label: string
  description: string
  icon: string
}> = [
  { value: 'light', label: '白天模式', description: '始终使用浅色外观', icon: 'lucide:sun' },
  { value: 'dark', label: '黑夜模式', description: '始终使用深色外观', icon: 'lucide:moon' },
  { value: 'auto', label: '跟随系统', description: '根据系统外观自动切换', icon: 'lucide:monitor' },
]

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

function selectThemeMode(mode: ThemeModePreference) {
  theme?.setMode(mode)
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
    savedDeviceName.value = deviceName.value.trim()
    savePath.value = settings.save_path || 'Download/FlashLAN'
  } catch (error) {
    settingsError.value = String(error).replace(/^Error:\s*/, '') || '设置加载失败'
    if (deviceStore.localDevice) {
      deviceName.value = deviceStore.localDevice.name
    }
    savedDeviceName.value = deviceName.value.trim()
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
    savedDeviceName.value = name
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

onMounted(() => {
  void loadSettings()
})
</script>

<template>
  <div class="fl-page fl-content-settings space-y-4 md:space-y-6">
    <PageHeader title="设置" description="管理本机与传输偏好" />

    <SCard
      class="overflow-hidden transition-opacity duration-150"
      :class="isLoading ? 'opacity-70' : ''"
      :ui="{ content: 'p-0!' }"
      :aria-busy="isLoading"
    >
      <div class="divide-y divide-border">
        <SettingsRow
          icon="lucide:monitor"
          title="设备名称"
          description="局域网内显示的名称"
          stacked
        >
          <template #control>
            <div class="flex w-full items-center gap-2 sm:w-auto">
              <SInput
                v-model="deviceName"
                class="h-11 w-full sm:h-8 sm:w-56"
                placeholder="输入设备名称"
                :disabled="isLoading || isSavingName"
                @keyup.enter="saveDeviceName"
              />
              <SButton
                size="sm"
                class="min-h-11 shrink-0 sm:min-h-8"
                :variant="canSaveDeviceName ? 'solid' : 'outline'"
                :disabled="isLoading || isSavingName || !canSaveDeviceName"
                @click="saveDeviceName"
              >
                {{ isSavingName ? '保存中...' : '保存' }}
              </SButton>
            </div>
          </template>
        </SettingsRow>

        <SettingsRow icon="lucide:folder" title="保存路径">
          <template #description>
            <span v-if="isMobile">文件将保存到 Download/FlashLAN</span>
            <span v-else class="block max-w-sm truncate font-mono" :title="savePath">
              {{ displayedSavePath || '加载中...' }}
            </span>
          </template>
          <template v-if="!isMobile" #control>
            <SButton
              variant="outline"
              size="sm"
              class="min-h-11 shrink-0 sm:min-h-8"
              :disabled="isLoading || isChoosingPath"
              @click="chooseSavePath"
            >
              <SIcon icon="lucide:folder-open" />
              {{ isChoosingPath ? '选择中...' : '选择路径' }}
            </SButton>
          </template>
        </SettingsRow>

        <SettingsRow
          icon="lucide:shield-check"
          title="可信设备自动接收"
          description="开启后仅可信设备发送的文件自动接收，其他设备仍会弹窗询问"
        >
          <template #control>
            <SSwitch v-model="autoReceive" class="ml-auto shrink-0" aria-label="可信设备自动接收" />
          </template>
        </SettingsRow>

        <SettingsRow
          icon="lucide:palette"
          title="主题模式"
          description="选择应用的外观显示方式"
          stacked
        >
          <template #control>
            <div
              class="flex w-full items-center rounded-lg border border-border bg-muted/40 p-1 sm:w-auto"
              role="radiogroup"
              aria-label="主题模式"
            >
              <SButton
                v-for="option in themeModeOptions"
                :key="option.value"
                role="radio"
                :aria-checked="themeMode === option.value"
                size="sm"
                :variant="themeMode === option.value ? 'soft' : 'ghost'"
                :color="themeMode === option.value ? 'primary' : 'secondary'"
                class="min-h-11 flex-1 px-2 sm:min-h-8 sm:flex-none"
                :title="option.description"
                @click="selectThemeMode(option.value)"
              >
                <SIcon :icon="option.icon" class="shrink-0" />
                <span class="truncate">{{ option.label }}</span>
              </SButton>
            </div>
          </template>
        </SettingsRow>

        <SettingsRow icon="lucide:plug" title="端口" description="发现 mDNS / 传输 17321">
          <template #control>
            <span class="rounded-md bg-muted px-2 py-1 font-mono text-xs">17321</span>
          </template>
        </SettingsRow>

        <SettingsRow icon="lucide:info" title="本机信息">
          <template #description>
            <span class="font-mono">
              {{ deviceStore.localDevice?.id || '-' }}
            </span>
          </template>
          <template #control>
            <span class="rounded-md bg-muted px-2 py-1 text-xs">
              {{ deviceStore.localDevice?.platform || '-' }}
            </span>
          </template>
        </SettingsRow>
      </div>
    </SCard>

    <Transition name="fl-state">
      <div
        v-if="settingsMessage || settingsError"
        class="flex items-center gap-2 text-xs"
        :class="settingsError ? 'text-destructive' : 'text-success'"
      >
        <SIcon :icon="settingsError ? 'lucide:circle-alert' : 'lucide:check-circle-2'" />
        {{ settingsError || settingsMessage }}
      </div>
    </Transition>

    <SSeparator />

    <div class="text-xs text-muted-foreground flex items-center gap-2">
      <SIcon icon="lucide:info" class="text-xs" />
      FlashLAN v{{ appVersion }} · by 蒋思宇
    </div>
  </div>
</template>
