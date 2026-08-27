<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { SCard } from '@/ui/components/card'
import { SButton } from '@/ui/components/button'
import { SDialog } from '@/ui/components/dialog'
import { SIcon } from '@/ui/components/icon'
import { SInput } from '@/ui/components/input'
import { isMobilePlatform, isTauri } from '@/utils/tauri'
import { useDeviceStore } from '@/stores/device'

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
const qrDataUrl = computed(() =>
  qrSvg.value ? `data:image/svg+xml;charset=utf-8,${encodeURIComponent(qrSvg.value)}` : '',
)
const onlineCount = computed(
  () => deviceStore.devices.filter(device => device.online !== false).length,
)

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

function setScanStatus(message: string, isError = false) {
  scanStatus.value = message
  scanStatusIsError.value = isError
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

async function cancelActiveScan() {
  if (!isScanning.value) return
  try {
    await cancelScanner?.()
  } catch {
    // The scanner may already have closed itself after a successful decode.
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
  }
}

function openScanFromMore() {
  showMoreActions.value = false
  void scanConnect()
}

function openAddDeviceFromMore() {
  showMoreActions.value = false
  openAddDevice()
}

function closeMoreActions() {
  showMoreActions.value = false
}

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
    deviceStore.addManualDevice(ip, port)
    await deviceStore.refreshManualStatus()
    setScanStatus(`已连接 ${ip}:${port}`)
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

let statusTimer: ReturnType<typeof setInterval> | undefined
let unlistenBackButton: (() => void) | undefined

onMounted(() => {
  if (isTauri()) {
    document.addEventListener('click', closeMoreActions)
    void listen('back-button', () => {
      if (showMoreActions.value) {
        closeMoreActions()
      } else if (isScanning.value) {
        void cancelActiveScan()
      }
    }).then(unlisten => {
      unlistenBackButton = unlisten as () => void
    })
  }
  void deviceStore.discover()
  void deviceStore.refreshManualStatus()
  // 手动设备没有 mDNS 在线通知，用定时探测保持状态新鲜。
  statusTimer = setInterval(() => {
    void deviceStore.refreshManualStatus()
  }, 15000)
})

onBeforeUnmount(() => {
  void cancelActiveScan()
  document.removeEventListener('click', closeMoreActions)
  unlistenBackButton?.()
  if (statusTimer) clearInterval(statusTimer)
})
</script>

<template>
  <div class="mx-auto flex w-full max-w-5xl flex-col gap-6 p-4 md:p-8">
    <div class="flex flex-col justify-between gap-4 sm:flex-row sm:items-start">
      <div class="min-w-0">
        <h1 class="text-2xl font-bold tracking-tight">附近设备</h1>
        <p class="mt-1 text-sm text-muted-foreground">发现、连接并管理同一局域网中的设备</p>
        <div class="mt-3 flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
          <span
            class="size-1.5 rounded-full"
            :class="deviceStore.isDiscovering ? 'animate-pulse bg-primary' : 'bg-success'"
          />
          <span>{{ deviceStore.isDiscovering ? '正在搜索附近设备' : '设备发现已开启' }}</span>
          <span class="text-border">·</span>
          <span>{{ onlineCount }} 台在线</span>
          <span
            v-if="scanStatus"
            class="min-w-0 max-w-52 truncate"
            :class="scanStatusIsError ? 'text-destructive' : 'text-success'"
            aria-live="polite"
            :title="scanStatus"
          >
            · {{ scanStatus }}
          </span>
        </div>
      </div>
      <div class="flex flex-wrap items-center gap-2 sm:shrink-0 sm:justify-end">
        <template v-if="isMobile">
          <div class="relative">
            <SButton
              variant="outline"
              :aria-expanded="showMoreActions"
              aria-haspopup="menu"
              @click.stop="showMoreActions = !showMoreActions"
            >
              <SIcon icon="lucide:ellipsis" />
              更多
            </SButton>
            <div
              v-if="showMoreActions"
              role="menu"
              class="absolute right-0 top-full z-20 mt-2 min-w-40 rounded-xl border border-border/80 bg-card p-1.5 shadow-lg"
              @click.stop
            >
              <button
                type="button"
                role="menuitem"
                class="flex w-full items-center gap-2 rounded-lg px-3 py-2.5 text-left text-xs transition-colors hover:bg-muted"
                :disabled="isScanning"
                @click="openScanFromMore"
              >
                <SIcon
                  icon="lucide:scan-line"
                  class="text-primary"
                  :class="isScanning ? 'animate-pulse' : ''"
                />
                扫码连接
              </button>
              <button
                type="button"
                role="menuitem"
                class="flex w-full items-center gap-2 rounded-lg px-3 py-2.5 text-left text-xs transition-colors hover:bg-muted"
                @click="openAddDeviceFromMore"
              >
                <SIcon icon="lucide:plus" class="text-primary" />
                添加设备
              </button>
            </div>
          </div>
        </template>
        <template v-else>
          <SButton variant="outline" @click="openQrDialog">
            <SIcon icon="lucide:qr-code" />
            二维码
          </SButton>
          <SButton variant="outline" @click="openAddDevice">
            <SIcon icon="lucide:plus" />
            添加设备
          </SButton>
        </template>
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

    <div v-else class="grid grid-cols-1 gap-3 xl:grid-cols-2">
      <SCard
        v-for="d in deviceStore.devices"
        :key="d.id"
        class="group relative overflow-hidden border-border/80 dark:border-border/10 bg-card/95 p-3!"
      >
        <div class="flex items-center justify-between gap-4">
          <div class="flex min-w-0 items-center gap-3">
            <div
              class="flex size-12 shrink-0 items-center justify-center rounded-2xl bg-gradient-to-br from-primary/15 via-primary/8 to-muted text-primary ring-1 ring-primary/10"
            >
              <SIcon
                :icon="
                  d.platform === 'windows'
                    ? 'lucide:monitor'
                    : d.platform === 'macos'
                      ? 'lucide:laptop'
                      : 'lucide:smartphone'
                "
                class="text-xl"
              />
            </div>
            <div class="min-w-0">
              <div class="truncate text-sm font-semibold leading-5">{{ d.name }}</div>
              <div class="mt-0.5 truncate font-mono text-[11px] text-muted-foreground">
                {{ d.ip }}:{{ d.port }}
              </div>
              <div class="mt-1 text-[11px] text-muted-foreground">
                {{ platformLabel(d.platform) }}
              </div>
            </div>
          </div>
          <span
            class="inline-flex shrink-0 items-center gap-1.5 rounded-full px-2 py-1 text-[10px] font-medium"
            :class="
              d.online === false ? 'bg-muted text-muted-foreground' : 'bg-success/10 text-success'
            "
          >
            <span
              class="size-1.5 rounded-full"
              :class="d.online === false ? 'bg-muted-foreground' : 'bg-success'"
            />
            {{ d.online === false ? '离线' : '在线' }}
          </span>
        </div>
        <div
          class="mt-3 flex items-center justify-between border-t border-border/70 dark:border-border/10 pt-3"
        >
          <span class="text-[11px] text-muted-foreground">局域网设备</span>
          <span v-if="d.online !== false" class="flex items-center gap-1 text-[11px] text-success">
            <SIcon icon="lucide:wifi" class="text-xs" />
            连接稳定
          </span>
          <button
            v-else
            type="button"
            class="flex items-center gap-1 text-[11px] text-primary hover:underline"
            @click="deviceStore.testConnection(d.ip, d.port).catch(() => {})"
          >
            <SIcon icon="lucide:wifi-off" class="text-xs" />
            点击重试连接
          </button>
        </div>
      </SCard>
    </div>

    <SCard
      v-if="deviceStore.localDevice"
      class="overflow-hidden border-border/80 dark:border-border/10 shadow-sm"
    >
      <template #header>
        <div class="flex w-full items-center justify-between gap-3">
          <span class="flex items-center gap-2 text-sm font-semibold">
            <SIcon icon="lucide:monitor" class="text-muted-foreground" />
            本机
          </span>
          <span class="text-[11px] text-muted-foreground">当前设备</span>
        </div>
      </template>
      <div class="flex items-center gap-3 rounded-xl bg-muted/35 p-3">
        <div
          class="flex size-11 shrink-0 items-center justify-center rounded-2xl bg-primary/10 text-primary"
        >
          <SIcon icon="lucide:laptop" class="text-lg" />
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
