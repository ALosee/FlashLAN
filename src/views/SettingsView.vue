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
import { SDialog } from '@/ui/components/dialog'
import { useDeviceStore } from '@/stores/device'
import { useTransferStore } from '@/stores/transfer'
import { isMobilePlatform, isTauri } from '@/utils/tauri'

const deviceStore = useDeviceStore()
const transferStore = useTransferStore()
const savePath = ref('')
const deviceName = ref('')
const isLoading = ref(true)
const isSavingName = ref(false)
const isChoosingPath = ref(false)
const settingsMessage = ref('')
const settingsError = ref('')
const isMobile = isMobilePlatform()
const theme = useTheme()
const themeMode = computed<ThemeModePreference>(() => theme?.mode.value ?? 'light')

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

// ---------- Trusted devices ----------
interface TrustedDeviceEntry {
  fingerprint: string
  name: string
  pairedAt: number
}

const ownFingerprint = ref('')
const trustedDevices = ref<TrustedDeviceEntry[]>([])
const showPairDialog = ref(false)
const pairFingerprint = ref('')
const pairName = ref('')

function loadTrustedDevices() {
  if (!isTauri()) return
  void invoke<Array<[string, { name: string; paired_at: number }]>>('list_trusted_devices')
    .then(entries => {
      trustedDevices.value = entries.map(([fingerprint, info]) => ({
        fingerprint,
        name: info.name,
        pairedAt: info.paired_at,
      }))
    })
    .catch((error: unknown) => console.warn('[FlashLAN] list_trusted_devices failed', error))
}

function openPairDialog() {
  pairFingerprint.value = ''
  pairName.value = ''
  showPairDialog.value = true
}

async function confirmPair() {
  const fingerprint = pairFingerprint.value.trim().toLowerCase().replace(/\s+/g, '')
  const name = pairName.value.trim()
  if (!/^[0-9a-f]{64}$/.test(fingerprint)) {
    settingsError.value = '指纹格式不正确，应为 64 位十六进制字符'
    return
  }
  if (!name) {
    settingsError.value = '请输入对方设备名称'
    return
  }
  clearFeedback()
  try {
    await invoke('trust_device', { fingerprint, name })
    settingsMessage.value = `已信任设备「${name}」`
    showPairDialog.value = false
    loadTrustedDevices()
  } catch (error) {
    settingsError.value = String(error).replace(/^Error:\s*/, '') || '信任设备失败'
  }
}

async function untrustDevice(entry: TrustedDeviceEntry) {
  clearFeedback()
  try {
    await invoke('remove_trusted_device', { fingerprint: entry.fingerprint })
    settingsMessage.value = `已移除对「${entry.name}」的信任`
    loadTrustedDevices()
  } catch (error) {
    settingsError.value = String(error).replace(/^Error:\s*/, '') || '移除信任失败'
  }
}

function formatPairTime(ms: number) {
  if (!ms) return ''
  try {
    return new Intl.DateTimeFormat('zh-CN', {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
    }).format(new Date(ms))
  } catch {
    return ''
  }
}

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

onMounted(() => {
  void loadSettings()
  if (isTauri()) {
    void invoke<string>('get_own_fingerprint')
      .then(fingerprint => {
        ownFingerprint.value = fingerprint
      })
      .catch(error => console.warn('[FlashLAN] get_own_fingerprint failed', error))
    loadTrustedDevices()
  }
})
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
              <div v-if="isMobile" class="text-xs text-muted-foreground">
                文件将保存到 Download/FlashLAN
              </div>
              <div
                v-else
                class="text-xs text-muted-foreground truncate max-w-[360px]"
                :title="savePath"
              >
                {{ displayedSavePath || '加载中...' }}
              </div>
            </div>
          </div>
          <SButton
            v-if="!isMobile"
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

        <div class="py-3 md:py-4 flex items-center justify-between gap-3">
          <div class="flex min-w-0 items-center gap-3">
            <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
              <SIcon icon="lucide:shield-check" />
            </div>
            <div class="min-w-0">
              <div class="text-sm font-medium">可信设备自动接收</div>
              <div class="text-xs text-muted-foreground truncate">
                开启后仅可信设备发送的文件自动接收，其他设备仍会弹窗询问
              </div>
            </div>
          </div>
          <SSwitch v-model="autoReceive" class="shrink-0" />
        </div>

        <div class="py-3 md:py-4 flex flex-col gap-3">
          <div class="flex items-start justify-between gap-3">
            <div class="flex min-w-0 items-center gap-3">
              <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
                <SIcon icon="lucide:fingerprint" />
              </div>
              <div class="min-w-0">
                <div class="text-sm font-medium">可信设备</div>
                <div class="text-xs text-muted-foreground truncate">
                  已配对 {{ trustedDevices.length }} 台 · 本机指纹
                  {{ ownFingerprint ? ownFingerprint.slice(0, 8) + '…' : '…' }}
                </div>
              </div>
            </div>
            <SButton
              variant="outline"
              size="sm"
              class="shrink-0"
              :disabled="!isTauri()"
              @click="openPairDialog"
            >
              <SIcon icon="lucide:plus" />
              添加
            </SButton>
          </div>

          <p class="text-[11px] text-muted-foreground pl-12 hidden sm:block">
            配对方法：在两台设备上分别打开本页，将对方「设置 → 关于」中显示的设备指纹复制到这里。
            传输全程 TLS 加密，指纹用于核实对方身份。
          </p>

          <div v-if="trustedDevices.length" class="space-y-2 sm:pl-12">
            <div
              v-for="entry in trustedDevices"
              :key="entry.fingerprint"
              class="flex items-center justify-between gap-3 rounded-lg bg-muted/40 px-3 py-2"
            >
              <div class="min-w-0">
                <div class="truncate text-sm font-medium">{{ entry.name }}</div>
                <div class="truncate font-mono text-[11px] text-muted-foreground">
                  {{ entry.fingerprint.slice(0, 16) }}…
                  <span v-if="entry.pairedAt">· 配对于 {{ formatPairTime(entry.pairedAt) }}</span>
                </div>
              </div>
              <SButton
                variant="ghost"
                color="destructive"
                size="sm"
                shape="square"
                class="size-8 shrink-0"
                aria-label="移除信任"
                title="移除信任"
                @click="untrustDevice(entry)"
              >
                <SIcon icon="lucide:trash-2" />
              </SButton>
            </div>
          </div>
          <div
            v-else-if="isTauri()"
            class="rounded-lg border border-dashed px-3 py-2.5 text-xs text-muted-foreground sm:ml-12 w-fit"
          >
            暂无可信设备，添加后可实现自动接收
          </div>
        </div>

        <div class="py-3 md:py-4 flex flex-col gap-3">
          <div class="flex items-center gap-3">
            <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
              <SIcon icon="lucide:palette" />
            </div>
            <div>
              <div class="text-sm font-medium">主题模式</div>
              <div class="text-xs text-muted-foreground">选择应用的外观显示方式</div>
            </div>
          </div>
          <div
            class="grid grid-cols-3 gap-1.5 pl-0 sm:gap-2 sm:pl-12"
            role="radiogroup"
            aria-label="主题模式"
          >
            <button
              v-for="option in themeModeOptions"
              :key="option.value"
              type="button"
              role="radio"
              :aria-checked="themeMode === option.value"
              class="flex min-h-16 flex-col items-center justify-center gap-1 rounded-lg border px-1.5 py-2 text-center transition-colors sm:min-h-0 sm:flex-row sm:justify-start sm:gap-3 sm:px-3 sm:py-2.5 sm:text-left"
              :class="
                themeMode === option.value
                  ? 'border-primary bg-primary/10 text-primary shadow-sm'
                  : 'border-border bg-muted/20 hover:bg-muted'
              "
              @click="selectThemeMode(option.value)"
            >
              <SIcon :icon="option.icon" class="text-base shrink-0" />
              <span class="min-w-0">
                <span class="block text-xs font-medium sm:text-sm">{{ option.label }}</span>
                <span class="hidden text-xs text-muted-foreground truncate sm:block">
                  {{ option.description }}
                </span>
              </span>
            </button>
          </div>
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
          <div class="flex flex-col items-start sm:items-end gap-1.5">
            <span class="text-xs bg-muted px-2 py-1 rounded">
              {{ deviceStore.localDevice?.platform || '-' }}
            </span>
            <span
              v-if="ownFingerprint"
              class="text-[11px] text-muted-foreground font-mono break-all max-w-[320px]"
              title="安全指纹：其他设备配对时核对这串字符"
            >
              指纹 {{ ownFingerprint.slice(0, 32) }}
              <br />
              {{ ownFingerprint.slice(32) }}
            </span>
          </div>
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

    <!-- Pairing dialog -->
    <SDialog
      v-model:open="showPairDialog"
      title="添加可信设备"
      description="输入对方设备显示的安全指纹并命名。配对后双向自动接收对方的文件。"
      size="sm"
      :show-confirm="true"
      :show-cancel="true"
      confirm-text="信任此设备"
      cancel-text="取消"
      @confirm="confirmPair"
      @cancel="showPairDialog = false"
    >
      <div class="space-y-4">
        <div class="space-y-1.5">
          <label for="pair-name" class="text-sm font-medium">设备名称</label>
          <SInput
            id="pair-name"
            v-model="pairName"
            placeholder="例如：Jack 的手机"
            autocomplete="off"
          />
        </div>
        <div class="space-y-1.5">
          <label for="pair-fp" class="text-sm font-medium">安全指纹</label>
          <SInput
            id="pair-fp"
            v-model="pairFingerprint"
            placeholder="64 位十六进制指纹"
            autocomplete="off"
            class="font-mono"
          />
          <p class="text-[11px] text-muted-foreground">在对方设备的「设置 → 本机信息」中查看</p>
        </div>
      </div>
    </SDialog>

    <SSeparator />

    <div class="text-xs text-muted-foreground flex items-center gap-2">
      <SIcon icon="lucide:info" class="text-xs" />
      FlashLAN v{{ appVersion }} · Tauri 2 · Vue 3 · SoybeanUI · UnoCSS · mDNS
    </div>
  </div>
</template>
