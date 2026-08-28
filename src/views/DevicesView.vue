<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { onBackButtonPress } from '@tauri-apps/api/app'
import { SDropdownMenu } from '@soybeanjs/ui'
import type { MenuOptionData, MenuUi } from '@soybeanjs/headless'
import { SCard } from '@/ui/components/card'
import { SButton, SButtonIcon } from '@/ui/components/button'
import { SDialog } from '@/ui/components/dialog'
import { SIcon } from '@/ui/components/icon'
import { SInput } from '@/ui/components/input'
import { isMobilePlatform, isTauri } from '@/utils/tauri'
import { type Device, useDeviceStore } from '@/stores/device'

const deviceStore = useDeviceStore()
const isMobile = isMobilePlatform()
const manualIp = ref('')
const manualPort = ref('17321')
const showAddDevice = ref(false)
const isTestingConnection = ref(false)
const connectionState = ref<'idle' | 'testing' | 'success' | 'error'>('idle')
const connectionMessage = ref('')
const testedEndpoint = ref('')
const showQrDialog = ref(false)
const qrSvg = ref('')
const isGeneratingQr = ref(false)
const isScanning = ref(false)
const scanStatus = ref('')
const scanStatusIsError = ref(false)
const showMoreActions = ref(false)
const removingDeviceId = ref('')
const updatingTrustedDeviceKey = ref('')
const retryingDeviceKey = ref('')
const showAliasDialog = ref(false)
const aliasDevice = ref<Device | null>(null)
const aliasInput = ref('')
const qrDataUrl = computed(() =>
  qrSvg.value ? `data:image/svg+xml;charset=utf-8,${encodeURIComponent(qrSvg.value)}` : '',
)
const onlineCount = computed(
  () => deviceStore.devices.filter(device => device.online !== false).length,
)

type MoreAction = 'scan' | 'add'

const moreActions = computed<MenuOptionData<MoreAction>[]>(() => [
  {
    label: '扫码连接',
    value: 'scan',
    icon: 'lucide:scan-line',
    disabled: isScanning.value,
  },
  {
    label: '添加设备',
    value: 'add',
    icon: 'lucide:plus',
  },
])

const moreMenuUi: Partial<MenuUi> = {
  popup:
    'w-32 max-w-[calc(100vw-1.5rem)] rounded-xl border border-border/80 bg-card p-1 shadow-lg dark:border-border/10',
  item: 'min-h-10 rounded-lg',
  itemIcon: 'size-3.5 shrink-0 text-primary',
}

const ipv4Pattern = /^(25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)){3}$/

const manualEndpoint = computed(() => `${manualIp.value.trim()}:${manualPort.value.trim()}`)
const canAddManualDevice = computed(
  () => connectionState.value === 'success' && testedEndpoint.value === manualEndpoint.value,
)

function getErrorMessage(error: unknown) {
  const message = String(error)
    .replace(/^Error:\s*/, '')
    .trim()
  return message || '连接失败，请确认设备地址和端口正确'
}

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
    connectionMessage.value = '连接成功，可以添加设备'
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

  if (!canAddManualDevice.value) await testManualConnection()
  if (!canAddManualDevice.value) return

  deviceStore.addManualDevice(input.ip, input.port)
  showAddDevice.value = false
}

async function syncTrustedDevices() {
  if (!isTauri()) return
  try {
    const entries =
      await invoke<Array<[string, { name: string; paired_at: number }]>>('list_trusted_devices')
    deviceStore.syncTrustedDevices(entries.map(([fingerprint]) => fingerprint))
  } catch (error) {
    console.warn('[FlashLAN] list_trusted_devices failed', error)
  }
}

function setScanStatus(message: string, isError = false) {
  scanStatus.value = message
  scanStatusIsError.value = isError
}

function clearScanStatus() {
  scanStatus.value = ''
  scanStatusIsError.value = false
}

async function openQrDialog() {
  showQrDialog.value = true
  isGeneratingQr.value = true
  qrSvg.value = ''
  try {
    qrSvg.value = await invoke<string>('generate_connect_qr')
  } catch (error) {
    console.error('[FlashLAN] generate QR failed', error)
  } finally {
    isGeneratingQr.value = false
  }
}

let cancelScanner: (() => Promise<void>) | undefined
type BackButtonListener = Awaited<ReturnType<typeof onBackButtonPress>>
let backButtonListener: BackButtonListener | undefined
let backButtonListenerPromise: Promise<BackButtonListener | undefined> | undefined

async function enableBackButtonHandling() {
  if (!isTauri() || !isMobile || backButtonListener) return
  if (backButtonListenerPromise) {
    await backButtonListenerPromise
    return
  }

  const listenerPromise = onBackButtonPress(() => {
    if (showMoreActions.value) {
      closeMoreActions()
    } else if (isScanning.value) {
      void cancelActiveScan()
    }
  })
    .then(listener => {
      if (showMoreActions.value || isScanning.value) {
        backButtonListener = listener
      } else {
        void listener.unregister()
      }
      return listener
    })
    .catch(error => {
      console.error('[FlashLAN] register back-button listener failed', error)
      return undefined
    })

  backButtonListenerPromise = listenerPromise
  await listenerPromise
  if (backButtonListenerPromise === listenerPromise) backButtonListenerPromise = undefined
}

function disableBackButtonHandling() {
  const listener = backButtonListener
  backButtonListener = undefined
  if (listener) void listener.unregister()
}

async function cancelActiveScan() {
  if (!isScanning.value) return
  try {
    await cancelScanner?.()
  } catch {
    // The scanner may already have closed itself after a successful decode.
  } finally {
    // The Android plugin closes the camera but may leave the scan promise
    // pending after cancellation, so reset the UI state explicitly here.
    isScanning.value = false
    cancelScanner = undefined
    disableBackButtonHandling()
  }
}

async function scanConnect() {
  if (!isTauri()) return
  isScanning.value = true
  setScanStatus('正在打开相机…')
  try {
    const { scan, cancel, checkPermissions, requestPermissions, Format } =
      await import('@tauri-apps/plugin-barcode-scanner')
    cancelScanner = cancel
    await enableBackButtonHandling()
    let granted = await checkPermissions()
    if (granted !== 'granted') granted = await requestPermissions()
    if (granted !== 'granted') {
      setScanStatus('未获得相机权限，请在系统设置中允许访问相机', true)
      return
    }
    setScanStatus('请将二维码放入取景框内')
    const result = await scan({ formats: [Format.QRCode] })
    if (result?.content) await applyScanResult(result.content)
    else setScanStatus('没有读取到二维码', true)
  } catch (error) {
    const message = getErrorMessage(error)
    if (!message.toLowerCase().includes('cancel')) {
      console.error('[FlashLAN] scanner failed', error)
      setScanStatus(message, true)
    }
  } finally {
    isScanning.value = false
    cancelScanner = undefined
    disableBackButtonHandling()
  }
}

function selectMoreAction(item: MenuOptionData<MoreAction>) {
  showMoreActions.value = false
  if (item.value === 'scan') {
    void scanConnect()
  } else {
    openAddDevice()
  }
}

function closeMoreActions() {
  showMoreActions.value = false
}

async function removeManualDevice(device: Device) {
  if (!device.isManual || removingDeviceId.value) return
  removingDeviceId.value = device.id
  try {
    deviceStore.removeManualDevice(device.ip, device.port)
    clearScanStatus()
  } catch (error) {
    setScanStatus(getErrorMessage(error), true)
  } finally {
    removingDeviceId.value = ''
  }
}

async function retryDevice(device: Device) {
  const deviceKey = `${device.ip}:${device.port}`
  if (retryingDeviceKey.value) return
  retryingDeviceKey.value = deviceKey
  try {
    await deviceStore.testConnection(device.ip, device.port)
    device.online = true
    clearScanStatus()
  } catch (error) {
    setScanStatus(getErrorMessage(error), true)
  } finally {
    retryingDeviceKey.value = ''
  }
}

function openAliasDialog(device: Device) {
  aliasDevice.value = device
  aliasInput.value = device.alias || ''
  showAliasDialog.value = true
}

function closeAliasDialog() {
  showAliasDialog.value = false
  aliasDevice.value = null
  aliasInput.value = ''
}

function saveDeviceAlias() {
  const device = aliasDevice.value
  if (!device) return
  deviceStore.setDeviceAlias(device.ip, device.port, aliasInput.value)
  closeAliasDialog()
}

async function toggleTrustedDevice(device: Device) {
  if (!isTauri() || updatingTrustedDeviceKey.value) return
  const deviceKey = `${device.ip}:${device.port}`
  const shouldTrust = !device.trusted
  updatingTrustedDeviceKey.value = deviceKey
  clearScanStatus()
  try {
    const fingerprint =
      device.fingerprint ||
      (await invoke<string>('get_peer_fingerprint', {
        targetIp: device.ip,
        targetPort: device.port,
      }))

    if (shouldTrust) {
      await invoke('trust_device', { fingerprint, name: deviceDisplayName(device) })
      // Keep the identity with the device record so this action remains
      // available after mDNS refreshes and app restarts.
      deviceStore.addManualDevice(device.ip, device.port, fingerprint, true)
      clearScanStatus()
    } else {
      await invoke('remove_trusted_device', { fingerprint })
      deviceStore.setManualDeviceTrusted(device.ip, device.port, false)
      clearScanStatus()
    }
  } catch (error) {
    setScanStatus(getErrorMessage(error), true)
  } finally {
    updatingTrustedDeviceKey.value = ''
  }
}

watch(showMoreActions, isMoreActionsOpen => {
  if (isMoreActionsOpen) {
    void enableBackButtonHandling()
  } else if (!isScanning.value) {
    disableBackButtonHandling()
  }
})

async function applyScanResult(raw: string) {
  const match = /^flashlan:\/\/([\d.]+):(\d+)#([0-9a-fA-F]{64})$/.exec(raw.trim())
  if (!match?.[1] || !match[2] || !match[3]) {
    setScanStatus('二维码不是有效的 FlashLAN 连接码，请重新扫描', true)
    return
  }
  const ip = match[1]
  const port = Number(match[2])
  const fingerprint = match[3].toLowerCase()
  setScanStatus('正在验证设备身份…')
  try {
    await invoke('verify_peer_fingerprint', { targetIp: ip, targetPort: port, fingerprint })
    await invoke('trust_device', { fingerprint, name: ip })
    deviceStore.addManualDevice(ip, port, fingerprint, true)
    await deviceStore.refreshManualStatus()
    clearScanStatus()
  } catch (error) {
    console.error('[FlashLAN] scan-connect failed', error)
    setScanStatus(getErrorMessage(error), true)
  }
}

function platformLabel(platform: string) {
  if (platform === 'macos') return 'macOS'
  if (platform === 'windows') return 'Windows'
  if (platform === 'android') return 'Android'
  if (platform === 'ios') return 'iPhone / iPad'
  if (platform === 'manual') return '手动添加'
  return platform
}

function deviceDisplayName(device: Device) {
  return device.alias || device.name
}

interface DeviceVisual {
  icon: string
  accent: string
  avatar: string
}

const deviceVisuals: Record<string, DeviceVisual> = {
  macos: {
    icon: 'lucide:laptop',
    accent: 'bg-indigo-300/75',
    avatar: 'bg-indigo-50/90 text-indigo-500 dark:bg-indigo-400/10 dark:text-indigo-300',
  },
  windows: {
    icon: 'lucide:monitor',
    accent: 'bg-sky-300/75',
    avatar: 'bg-sky-50/90 text-sky-500 dark:bg-sky-400/10 dark:text-sky-300',
  },
  android: {
    icon: 'lucide:smartphone',
    accent: 'bg-emerald-300/75',
    avatar: 'bg-emerald-50/90 text-emerald-500 dark:bg-emerald-400/10 dark:text-emerald-300',
  },
  ios: {
    icon: 'lucide:smartphone',
    accent: 'bg-rose-300/75',
    avatar: 'bg-rose-50/90 text-rose-500 dark:bg-rose-400/10 dark:text-rose-300',
  },
  manual: {
    icon: 'lucide:router',
    accent: 'bg-rose-300/75',
    avatar: 'bg-rose-50/90 text-rose-500 dark:bg-rose-400/10 dark:text-rose-300',
  },
  default: {
    icon: 'lucide:smartphone',
    accent: 'bg-violet-300/75',
    avatar: 'bg-violet-50/90 text-violet-500 dark:bg-violet-400/10 dark:text-violet-300',
  },
}

function deviceVisual(platform: string): DeviceVisual {
  return deviceVisuals[platform] ?? deviceVisuals.default!
}

interface DeviceGroup {
  key: 'manual' | 'discovered'
  label: string
  icon: string
  iconSurface: string
  count: string
  devices: Device[]
}

const manualDevices = computed(() => deviceStore.devices.filter(device => device.isManual))
const discoveredDevices = computed(() => deviceStore.devices.filter(device => !device.isManual))
const deviceGroups = computed<DeviceGroup[]>(() => {
  const groups: DeviceGroup[] = [
    {
      key: 'discovered',
      label: '自动发现',
      icon: 'lucide:scan-search',
      iconSurface: 'bg-sky-50/90 text-sky-500 dark:bg-sky-400/10 dark:text-sky-300',
      count: 'bg-sky-50/90 text-sky-600 dark:bg-sky-400/10 dark:text-sky-300',
      devices: discoveredDevices.value,
    },
    {
      key: 'manual',
      label: '手动添加',
      icon: 'lucide:plus-circle',
      iconSurface: 'bg-rose-50/90 text-rose-500 dark:bg-rose-400/10 dark:text-rose-300',
      count: 'bg-rose-50/90 text-rose-600 dark:bg-rose-400/10 dark:text-rose-300',
      devices: manualDevices.value,
    },
  ]
  return groups.filter(group => group.devices.length > 0)
})

let statusTimer: ReturnType<typeof setInterval> | undefined

onMounted(() => {
  void deviceStore.discover()
  void deviceStore.refreshManualStatus()
  void syncTrustedDevices()
  // 手动设备没有 mDNS 在线通知，用定时探测保持状态新鲜。
  statusTimer = setInterval(() => {
    void deviceStore.refreshManualStatus()
  }, 15000)
})

onBeforeUnmount(() => {
  void cancelActiveScan()
  disableBackButtonHandling()
  if (statusTimer) clearInterval(statusTimer)
})
</script>

<template>
  <div class="mx-auto flex w-full max-w-5xl flex-col gap-6 p-4 md:p-8">
    <div class="flex flex-col justify-between gap-4 sm:flex-row sm:items-start">
      <div class="min-w-0 flex-1">
        <div class="flex items-start justify-between gap-3">
          <div class="min-w-0">
            <h1 class="text-2xl font-bold tracking-tight">附近设备</h1>
            <p class="mt-1 text-sm text-muted-foreground">发现、连接并管理同一局域网中的设备</p>
          </div>
          <div v-if="isMobile" class="flex shrink-0 items-center gap-2">
            <SDropdownMenu
              v-model:open="showMoreActions"
              :items="moreActions"
              placement="bottom-end"
              :show-arrow="false"
              :ui="moreMenuUi"
              @select="selectMoreAction"
            >
              <template #trigger>
                <SButtonIcon
                  color="primary"
                  variant="soft"
                  icon="lucide:ellipsis"
                  aria-label="更多操作"
                  title="更多操作"
                  class="size-10 rounded-xl"
                />
              </template>
            </SDropdownMenu>
            <SButtonIcon
              color="primary"
              variant="solid"
              icon="lucide:refresh-cw"
              :icon-class="deviceStore.isDiscovering ? 'animate-spin' : ''"
              aria-label="刷新附近设备"
              title="刷新附近设备"
              :disabled="deviceStore.isDiscovering"
              class="size-10 rounded-xl"
              @click="deviceStore.discover()"
            />
          </div>
        </div>
        <div class="mt-3 flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
          <span
            class="size-1.5 rounded-full"
            :class="deviceStore.isDiscovering ? 'animate-pulse bg-primary' : 'bg-success'"
          />
          <span>{{ deviceStore.isDiscovering ? '正在搜索附近设备' : '设备发现已开启' }}</span>
          <span class="text-border">·</span>
          <span>{{ onlineCount }} 台在线</span>
          <span
            v-if="scanStatus && scanStatusIsError"
            class="min-w-0 max-w-52 truncate"
            :class="scanStatusIsError ? 'text-destructive' : 'text-success'"
            aria-live="polite"
            :title="scanStatus"
          >
            · {{ scanStatus }}
          </span>
        </div>
      </div>
      <div v-if="!isMobile" class="flex flex-wrap items-center gap-2 sm:shrink-0 sm:justify-end">
        <SButton variant="outline" @click="openQrDialog">
          <SIcon icon="lucide:qr-code" />
          二维码
        </SButton>
        <SButton variant="outline" @click="openAddDevice">
          <SIcon icon="lucide:plus" />
          添加设备
        </SButton>
        <SButton
          class="shadow-sm"
          :disabled="deviceStore.isDiscovering"
          @click="deviceStore.discover()"
        >
          <SIcon
            icon="lucide:refresh-cw"
            :class="deviceStore.isDiscovering ? 'animate-spin' : ''"
          />
          {{ deviceStore.isDiscovering ? '扫描中' : '刷新' }}
        </SButton>
      </div>
    </div>

    <div
      v-if="deviceStore.error"
      class="flex items-start gap-2 rounded-xl border border-destructive/15 bg-destructive/8 px-4 py-3 text-sm text-destructive"
    >
      <SIcon icon="lucide:circle-alert" class="mt-0.5 shrink-0" />
      {{ deviceStore.error }}
    </div>

    <div
      v-if="deviceStore.devices.length === 0 && !deviceStore.isDiscovering"
      class="rounded-2xl border border-dashed border-border/80 dark:border-border/10 bg-card/60 p-10 text-center"
    >
      <div
        class="mx-auto flex size-14 items-center justify-center rounded-2xl bg-primary/10 text-primary"
      >
        <SIcon icon="lucide:scan-search" class="text-2xl" />
      </div>
      <div class="mt-4 text-sm font-semibold">未发现设备</div>
      <div class="mt-1 text-xs text-muted-foreground">
        请确保另一台设备已启动 FlashLAN 并在同一 WiFi
      </div>
    </div>

    <div v-else-if="deviceStore.devices.length" class="space-y-5">
      <section
        v-for="group in deviceGroups"
        :key="group.key"
        class="space-y-3"
        :aria-labelledby="`${group.key}-devices-title`"
      >
        <div class="flex items-center gap-2 px-1">
          <div
            class="flex size-8 shrink-0 items-center justify-center rounded-xl"
            :class="group.iconSurface"
          >
            <SIcon :icon="group.icon" class="text-sm" />
          </div>
          <h2 :id="`${group.key}-devices-title`" class="text-sm font-semibold">
            {{ group.label }}
          </h2>
          <span class="rounded-full px-2 py-0.5 text-[10px] font-medium" :class="group.count">
            {{ group.devices.length }} 台
          </span>
        </div>

        <div class="grid grid-cols-1 gap-3 xl:grid-cols-2">
          <SCard
            v-for="d in group.devices"
            :key="d.id"
            class="relative overflow-hidden rounded-3xl border-0 bg-muted/50 p-0!"
          >
            <div class="relative p-3 sm:p-4">
              <div class="relative flex items-start gap-3.5">
                <div class="flex w-14 shrink-0 flex-col items-center gap-1">
                  <div
                    class="relative flex size-14 items-center justify-center rounded-[1.5rem]"
                    :class="deviceVisual(d.platform).avatar"
                  >
                    <SIcon :icon="deviceVisual(d.platform).icon" class="relative text-2xl" />
                  </div>
                  <div
                    class="flex max-w-full items-center gap-1 text-center text-[10px] leading-4 text-muted-foreground"
                  >
                    <span
                      class="size-1.5 shrink-0 rounded-full"
                      :class="deviceVisual(d.platform).accent"
                    />
                    <span class="truncate">{{ platformLabel(d.platform) }}</span>
                  </div>
                </div>
                <div class="min-w-0 flex-1">
                  <div class="flex min-w-0 items-center gap-1" :class="d.isManual ? 'pr-10' : ''">
                    <div class="truncate text-[15px] font-semibold leading-5">
                      {{ deviceDisplayName(d) }}
                    </div>
                    <SButtonIcon
                      icon="lucide:pencil"
                      color="primary"
                      variant="ghost"
                      class="size-6 shrink-0 rounded-full"
                      aria-label="编辑设备别名"
                      title="编辑设备别名"
                      @click="openAliasDialog(d)"
                    />
                  </div>
                  <div class="mt-1 truncate font-mono text-[11px] text-muted-foreground">
                    {{ d.ip }}:{{ d.port }}
                  </div>
                  <div class="mt-2 flex min-w-0 flex-wrap items-center gap-1.5 text-[11px]">
                    <span
                      v-if="d.trusted"
                      class="inline-flex shrink-0 items-center gap-1 rounded-full bg-success/10 px-1.5 py-0.5 text-[10px] font-medium text-success"
                    >
                      <SIcon icon="lucide:shield-check" class="text-[11px]" />
                      可信
                    </span>
                    <span
                      class="inline-flex shrink-0 items-center gap-1.5 rounded-full px-2 py-1 text-[10px] font-medium"
                      :class="
                        d.online === false
                          ? 'bg-muted text-muted-foreground'
                          : 'bg-success/10 text-success'
                      "
                    >
                      <span
                        class="size-1.5 rounded-full"
                        :class="d.online === false ? 'bg-muted-foreground' : 'bg-success'"
                      />
                      {{ d.online === false ? '离线' : '在线' }}
                    </span>
                  </div>
                </div>
                <SButton
                  v-if="d.isManual"
                  variant="ghost"
                  color="destructive"
                  size="sm"
                  shape="square"
                  class="absolute right-0 top-0 size-8 shrink-0 rounded-full"
                  :disabled="removingDeviceId === d.id"
                  aria-label="删除设备"
                  title="删除设备（仅移除本机记录）"
                  @click="removeManualDevice(d)"
                >
                  <SIcon
                    :icon="removingDeviceId === d.id ? 'lucide:loader-circle' : 'lucide:trash-2'"
                    :class="removingDeviceId === d.id ? 'animate-spin' : ''"
                  />
                </SButton>
              </div>
              <div
                class="mt-4 flex items-end justify-between gap-3 border-t border-border/55 pt-3 dark:border-border/10"
              >
                <div class="flex min-w-0 items-center gap-1.5 text-[11px] text-muted-foreground">
                  <SIcon icon="lucide:network" class="shrink-0 text-xs opacity-70" />
                  <span>局域网设备</span>
                  <span v-if="d.online === false" class="text-border">·</span>
                  <button
                    v-if="d.online === false"
                    type="button"
                    class="inline-flex min-w-0 items-center gap-1 truncate text-primary hover:underline disabled:cursor-not-allowed disabled:opacity-60"
                    :disabled="retryingDeviceKey === `${d.ip}:${d.port}`"
                    :aria-busy="retryingDeviceKey === `${d.ip}:${d.port}`"
                    @click="retryDevice(d)"
                  >
                    <SIcon
                      :icon="
                        retryingDeviceKey === `${d.ip}:${d.port}`
                          ? 'lucide:loader-circle'
                          : 'lucide:refresh-cw'
                      "
                      :class="retryingDeviceKey === `${d.ip}:${d.port}` ? 'animate-spin' : ''"
                    />
                    {{ retryingDeviceKey === `${d.ip}:${d.port}` ? '重试中…' : '点击重试' }}
                  </button>
                </div>
                <SButton
                  v-if="isTauri()"
                  variant="soft"
                  :color="d.trusted ? 'destructive' : 'primary'"
                  size="sm"
                  class="h-8 shrink-0 rounded-full px-3 text-[11px]"
                  :disabled="updatingTrustedDeviceKey === `${d.ip}:${d.port}`"
                  :aria-label="d.trusted ? '移除可信设备' : '设为可信设备'"
                  :title="d.trusted ? '移除可信设备（不会删除设备记录）' : '设为可信设备'"
                  @click="toggleTrustedDevice(d)"
                >
                  <SIcon
                    :icon="
                      updatingTrustedDeviceKey === `${d.ip}:${d.port}`
                        ? 'lucide:loader-circle'
                        : d.trusted
                          ? 'lucide:shield-check'
                          : 'lucide:shield-plus'
                    "
                    :class="updatingTrustedDeviceKey === `${d.ip}:${d.port}` ? 'animate-spin' : ''"
                  />
                  {{
                    updatingTrustedDeviceKey === `${d.ip}:${d.port}`
                      ? '处理中'
                      : d.trusted
                        ? '移除可信'
                        : '设为可信'
                  }}
                </SButton>
              </div>
            </div>
          </SCard>
        </div>
      </section>
    </div>

    <section v-if="deviceStore.localDevice" class="space-y-3" aria-labelledby="local-devices-title">
      <div class="flex items-center gap-2 px-1">
        <div
          class="flex size-8 shrink-0 items-center justify-center rounded-xl bg-violet-50/90 text-violet-500 dark:bg-violet-400/10 dark:text-violet-300"
        >
          <SIcon icon="lucide:monitor" class="text-sm" />
        </div>
        <h2 id="local-devices-title" class="text-sm font-semibold">本机</h2>
        <span
          class="rounded-full bg-violet-50/90 px-2 py-0.5 text-[10px] font-medium text-violet-600 dark:bg-violet-400/10 dark:text-violet-300"
        >
          1 台
        </span>
        <span class="ml-auto shrink-0 text-[11px] text-muted-foreground">当前设备</span>
      </div>

      <SCard class="overflow-hidden rounded-3xl border-0 bg-muted/50 p-0!">
        <div class="flex items-center gap-3 p-3 sm:p-4">
          <div
            class="flex size-12 shrink-0 items-center justify-center rounded-[1.5rem] bg-violet-50/90 text-violet-500 dark:bg-violet-400/10 dark:text-violet-300"
          >
            <SIcon icon="lucide:laptop" class="text-xl" />
          </div>
          <div class="min-w-0 flex-1">
            <div class="truncate text-sm font-semibold">{{ deviceStore.localDevice.name }}</div>
            <div class="mt-1 truncate font-mono text-[11px] text-muted-foreground">
              {{ deviceStore.localDevice.ip }}:{{ deviceStore.localDevice.port }} ·
              {{ platformLabel(deviceStore.localDevice.platform) }}
            </div>
          </div>
          <span
            class="inline-flex shrink-0 items-center gap-1.5 rounded-full bg-success/10 px-2 py-1 text-[10px] font-medium text-success"
          >
            <span class="size-1.5 rounded-full bg-success" />
            在线
          </span>
        </div>
      </SCard>
    </section>

    <SDialog
      v-model:open="showQrDialog"
      title="扫码连接"
      description="用手机上的 FlashLAN 扫描此二维码，即可添加本设备并自动完成配对。"
      size="sm"
      :show-confirm="false"
      :show-cancel="true"
      cancel-text="关闭"
    >
      <div class="flex flex-col items-center gap-3 py-2">
        <img
          v-if="qrDataUrl"
          :src="qrDataUrl"
          alt="FlashLAN 连接二维码"
          class="size-56 rounded-xl"
        />
        <div
          v-else-if="isGeneratingQr"
          class="flex h-56 items-center text-sm text-muted-foreground"
        >
          正在生成…
        </div>
        <div v-else class="flex h-56 items-center text-sm text-destructive">生成失败</div>
      </div>
    </SDialog>

    <SDialog
      v-model:open="showAliasDialog"
      title="设置设备别名"
      description="设置一个便于识别的显示名称，留空可恢复原名称。"
      size="sm"
      :show-fullscreen="false"
      :show-confirm="false"
      :show-cancel="false"
    >
      <form id="device-alias-form" class="space-y-3" @submit.prevent="saveDeviceAlias">
        <label for="device-alias" class="text-sm font-medium">设备别名</label>
        <SInput
          id="device-alias"
          v-model="aliasInput"
          autofocus
          :maxlength="32"
          autocomplete="off"
          placeholder="例如：我的 MacBook"
        />
      </form>

      <template #footer>
        <SButton variant="ghost" @click="closeAliasDialog">取消</SButton>
        <SButton type="submit" form="device-alias-form" @click="saveDeviceAlias">保存</SButton>
      </template>
    </SDialog>

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
  </div>
</template>
