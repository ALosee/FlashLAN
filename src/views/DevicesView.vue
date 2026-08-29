<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { onBackButtonPress } from '@tauri-apps/api/app'
import { SDropdownMenu } from '@soybeanjs/ui'
import type { MenuOptionData, MenuUi } from '@soybeanjs/headless'
import { SButton, SButtonIcon } from '@/ui/components/button'
import { SDialog } from '@/ui/components/dialog'
import { SIcon } from '@/ui/components/icon'
import { SInput } from '@/ui/components/input'
import {
  DeviceListSkeleton,
  DeviceRow,
  EmptyState,
  PageHeader,
  SectionHeader,
  StatusIndicator,
} from '@/ui/patterns'
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

type MoreAction = 'connect' | 'add'

const moreActions = computed<MenuOptionData<MoreAction>[]>(() => [
  {
    label: isMobile ? '扫码连接' : '显示二维码',
    value: 'connect',
    icon: isMobile ? 'lucide:scan-line' : 'lucide:qr-code',
    disabled: isMobile && isScanning.value,
  },
  {
    label: '添加设备',
    value: 'add',
    icon: 'lucide:plus',
  },
])

const moreMenuUi: Partial<MenuUi> = {
  popup: 'w-40 max-w-[calc(100vw-1.5rem)] rounded-lg border border-border bg-card p-1 shadow-lg',
  item: 'min-h-11 rounded-lg sm:min-h-8',
  itemIcon: 'size-3.5 shrink-0 text-primary',
}

type DeviceAction = 'alias' | 'trust' | 'remove'

function deviceActions(device: Device): MenuOptionData<DeviceAction>[] {
  const actions: MenuOptionData<DeviceAction>[] = [
    {
      label: '设置别名',
      value: 'alias',
      icon: 'lucide:pencil',
    },
  ]

  if (isTauri()) {
    actions.push({
      label: device.trusted ? '移除可信' : '设为可信',
      value: 'trust',
      icon: device.trusted ? 'lucide:shield-minus' : 'lucide:shield-plus',
      disabled: Boolean(updatingTrustedDeviceKey.value),
    })
  }

  if (device.isManual) {
    actions.push({
      label: '删除设备',
      value: 'remove',
      icon: 'lucide:trash-2',
      disabled: Boolean(removingDeviceId.value),
    })
  }

  return actions
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
  if (item.value === 'connect') {
    if (isMobile) void scanConnect()
    else openQrDialog()
  } else {
    openAddDevice()
  }
}

function selectDeviceAction(item: MenuOptionData<DeviceAction>, device: Device) {
  if (item.value === 'alias') {
    openAliasDialog(device)
  } else if (item.value === 'trust') {
    void toggleTrustedDevice(device)
  } else {
    void removeManualDevice(device)
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

function deviceIcon(platform: string) {
  if (platform === 'macos') return 'lucide:laptop'
  if (platform === 'windows') return 'lucide:monitor'
  if (platform === 'android' || platform === 'ios') return 'lucide:smartphone'
  if (platform === 'manual') return 'lucide:router'
  return 'lucide:smartphone'
}

interface DeviceGroup {
  key: 'manual' | 'discovered'
  label: string
  icon: string
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
      devices: discoveredDevices.value,
    },
    {
      key: 'manual',
      label: '手动添加',
      icon: 'lucide:plus-circle',
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
  <div class="fl-page fl-content-list flex flex-col gap-6">
    <PageHeader
      title="附近设备"
      description="发现、连接并管理同一局域网中的设备"
      mobile-actions-inline
    >
      <template #after-title>
        <div class="ml-auto flex shrink-0 items-center gap-2 md:hidden">
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
                aria-label="更多设备操作"
                title="更多设备操作"
                class="size-11 rounded-lg"
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
            class="size-11 rounded-lg"
            @click="deviceStore.discover()"
          />
        </div>
      </template>

      <template #status>
        <StatusIndicator
          :label="deviceStore.isDiscovering ? '正在搜索附近设备' : '设备发现已开启'"
          :tone="deviceStore.isDiscovering ? 'primary' : 'success'"
          :pulse="deviceStore.isDiscovering"
          live="polite"
        />
        <span class="text-xs text-muted-foreground">·</span>
        <span class="text-xs text-muted-foreground">{{ onlineCount }} 台在线</span>
        <StatusIndicator
          v-if="scanStatus && scanStatusIsError"
          :label="scanStatus"
          tone="destructive"
          live="polite"
        />
      </template>

      <template #actions>
        <SButton variant="outline" @click="openQrDialog">
          <SIcon icon="lucide:qr-code" />
          二维码
        </SButton>
        <SButton variant="outline" @click="openAddDevice">
          <SIcon icon="lucide:plus" />
          添加设备
        </SButton>
        <SButton :disabled="deviceStore.isDiscovering" @click="deviceStore.discover()">
          <SIcon
            icon="lucide:refresh-cw"
            :class="deviceStore.isDiscovering ? 'animate-spin' : ''"
          />
          {{ deviceStore.isDiscovering ? '扫描中' : '刷新' }}
        </SButton>
      </template>
    </PageHeader>

    <Transition name="fl-state">
      <div
        v-if="deviceStore.error"
        class="flex items-start gap-2 rounded-xl border border-destructive/15 bg-destructive/8 px-4 py-3 text-sm text-destructive"
      >
        <SIcon icon="lucide:circle-alert" class="mt-1 shrink-0" />
        {{ deviceStore.error }}
      </div>
    </Transition>

    <Transition name="fl-state" mode="out-in">
      <DeviceListSkeleton
        v-if="deviceStore.isDiscovering"
        key="loading"
        :rows="Math.max(deviceStore.devices.length, 3)"
      />

      <EmptyState
        v-else-if="deviceStore.devices.length === 0"
        key="empty"
        icon="lucide:scan-search"
        title="未发现设备"
        description="请确保另一台设备已启动 FlashLAN，并连接到同一 WiFi"
      />

      <div v-else key="devices" class="space-y-6">
        <section
          v-for="group in deviceGroups"
          :key="group.key"
          class="space-y-3"
          :aria-labelledby="`${group.key}-devices-title`"
        >
          <SectionHeader
            :id="`${group.key}-devices-title`"
            :icon="group.icon"
            :title="group.label"
            :count="`${group.devices.length} 台`"
          />

          <TransitionGroup
            name="fl-list"
            tag="div"
            class="divide-y divide-border overflow-hidden rounded-lg border border-border bg-card"
          >
            <DeviceRow
              v-for="d in group.devices"
              :key="d.id"
              :icon="d.isManual ? 'lucide:router' : deviceIcon(d.platform)"
              :name="deviceDisplayName(d)"
              :address="`${d.ip}:${d.port}`"
              :platform="d.isManual ? '' : platformLabel(d.platform)"
              :source="group.label"
              :online="d.online !== false"
              :trusted="d.trusted"
            >
              <template #actions>
                <SButton
                  v-if="d.online === false"
                  variant="link"
                  size="sm"
                  class="min-h-11 min-w-11 px-0 sm:min-h-8 sm:min-w-0"
                  :disabled="retryingDeviceKey === `${d.ip}:${d.port}`"
                  :aria-busy="retryingDeviceKey === `${d.ip}:${d.port}`"
                  :aria-label="
                    retryingDeviceKey === `${d.ip}:${d.port}` ? '正在重试连接' : '重试连接'
                  "
                  title="重试连接"
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
                  <span class="hidden sm:inline">
                    {{ retryingDeviceKey === `${d.ip}:${d.port}` ? '重试中…' : '重试' }}
                  </span>
                </SButton>
                <SDropdownMenu
                  :items="deviceActions(d)"
                  placement="bottom-end"
                  :show-arrow="false"
                  :ui="moreMenuUi"
                  @select="item => selectDeviceAction(item, d)"
                >
                  <template #trigger>
                    <SButtonIcon
                      icon="lucide:ellipsis"
                      variant="ghost"
                      class="size-11 sm:size-8"
                      :aria-label="`管理设备：${deviceDisplayName(d)}`"
                      title="更多设备操作"
                    />
                  </template>
                </SDropdownMenu>
              </template>
            </DeviceRow>
          </TransitionGroup>
        </section>
      </div>
    </Transition>

    <section v-if="deviceStore.localDevice" class="space-y-3" aria-labelledby="local-devices-title">
      <SectionHeader id="local-devices-title" icon="lucide:monitor" title="本机" count="1 台" />

      <div class="overflow-hidden rounded-lg border border-border bg-card">
        <DeviceRow
          :icon="deviceIcon(deviceStore.localDevice.platform)"
          :name="deviceStore.localDevice.name"
          :address="`${deviceStore.localDevice.ip}:${deviceStore.localDevice.port}`"
          :platform="platformLabel(deviceStore.localDevice.platform)"
          :online="true"
          current
        />
      </div>
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
