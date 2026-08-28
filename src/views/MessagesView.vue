<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { SButton } from '@/ui/components/button'
import { SCard } from '@/ui/components/card'
import { SIcon } from '@/ui/components/icon'
import { useDeviceStore } from '@/stores/device'
import { type TextMessageItem, useTransferStore } from '@/stores/transfer'

interface MessagePeer {
  key: string
  name: string
  sourceName?: string
  ip?: string
  port?: number
  online: boolean
}

const deviceStore = useDeviceStore()
const transferStore = useTransferStore()
const router = useRouter()

const selectedPeerKey = ref('')
const draftText = ref('')
const isSendingText = ref(false)
const sendError = ref('')
const maxTextBytes = 512 * 1024

const peers = computed<MessagePeer[]>(() => {
  const knownPeers: MessagePeer[] = deviceStore.devices.map(device => ({
    key: `device:${device.id}`,
    name: device.alias || device.name,
    sourceName: device.name,
    ip: device.ip,
    port: device.port,
    online: device.online !== false,
  }))
  const knownNames = new Set(
    knownPeers.flatMap(peer => [peer.name, peer.sourceName, peer.ip].filter(Boolean)),
  )
  const historyPeers = Array.from(new Set(transferStore.textMessages.map(message => message.peer)))

  return [
    ...knownPeers,
    ...historyPeers
      .filter(peer => !knownNames.has(peer))
      .map(peer => ({
        key: `history:${peer}`,
        name: peer,
        online: false,
      })),
  ]
})

function matchesPeer(message: TextMessageItem, peer: MessagePeer) {
  return [peer.ip, peer.name, peer.sourceName].includes(message.peer)
}

const peerRows = computed(() =>
  peers.value.map(peer => {
    const latestMessage = transferStore.textMessages.find(message => matchesPeer(message, peer))
    return {
      ...peer,
      latestText: latestMessage?.text || '暂无消息，开始发送一条消息吧',
      latestAt: latestMessage?.createdAt,
    }
  }),
)

const selectedPeer = computed(() => peerRows.value.find(peer => peer.key === selectedPeerKey.value))
const selectedMessages = computed(() =>
  selectedPeer.value
    ? transferStore.textMessages
        .filter(message => matchesPeer(message, selectedPeer.value!))
        .reverse()
    : [],
)
const draftTextBytes = computed(() => new TextEncoder().encode(draftText.value).length)
const canSendText = computed(() =>
  Boolean(
    selectedPeer.value?.ip &&
    selectedPeer.value.online &&
    draftText.value.trim() &&
    draftTextBytes.value <= maxTextBytes &&
    !isSendingText.value,
  ),
)
const onlineCount = computed(
  () => deviceStore.devices.filter(device => device.online !== false).length,
)

watch(
  peerRows,
  nextPeers => {
    if (!nextPeers.some(peer => peer.key === selectedPeerKey.value)) {
      selectedPeerKey.value = nextPeers.find(peer => peer.online)?.key || nextPeers[0]?.key || ''
    }
  },
  { immediate: true },
)

watch(selectedPeerKey, () => {
  sendError.value = ''
})

function formatMessageTime(createdAt?: number) {
  if (!createdAt) return ''
  try {
    return new Intl.DateTimeFormat('zh-CN', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    }).format(new Date(createdAt))
  } catch {
    return ''
  }
}

function copyText(text: string) {
  void navigator.clipboard?.writeText(text).catch(() => {})
}

function onTextComposerKeydown(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
    event.preventDefault()
    void sendTextToSelectedPeer()
  }
}

async function sendTextToSelectedPeer() {
  const peer = selectedPeer.value
  const text = draftText.value.trim()
  if (!peer?.ip || !peer.port || !text || !canSendText.value) return

  sendError.value = ''
  isSendingText.value = true
  try {
    await transferStore.sendText(text, peer.ip, peer.port)
    draftText.value = ''
  } catch (error) {
    console.error('[FlashLAN] send text failed', error)
    sendError.value = String(error).replace(/^Error:\s*/, '') || '消息发送失败，请稍后重试'
  } finally {
    isSendingText.value = false
  }
}

async function refreshDevices() {
  if (deviceStore.isDiscovering) return
  await deviceStore.discover()
  await deviceStore.refreshManualStatus()
}

function openDevices() {
  void router.push('/devices')
}

let statusTimer: ReturnType<typeof setInterval> | undefined

onMounted(async () => {
  await deviceStore.fetchLocal()
  await deviceStore.discover()
  await transferStore.ensureListener()
  await deviceStore.refreshManualStatus()
  statusTimer = setInterval(() => {
    void deviceStore.refreshManualStatus()
  }, 15000)
})

onBeforeUnmount(() => {
  if (statusTimer) clearInterval(statusTimer)
})
</script>

<template>
  <div class="mx-auto flex w-full max-w-5xl flex-col gap-6 p-4 md:p-8">
    <div class="flex flex-col gap-1">
      <h1 class="text-2xl font-bold tracking-tight">消息</h1>
      <p class="text-sm text-muted-foreground">与附近设备发送文字消息，后续可以继续扩展为聊天</p>
    </div>

    <div
      v-if="deviceStore.error"
      class="flex items-start gap-2 rounded-xl border border-destructive/15 bg-destructive/8 px-4 py-3 text-sm text-destructive"
      role="alert"
    >
      <SIcon icon="lucide:circle-alert" class="mt-0.5 shrink-0" />
      <span>{{ deviceStore.error }}</span>
    </div>

    <div class="grid min-h-0 grid-cols-1 gap-4 md:grid-cols-[15rem_minmax(0,1fr)]">
      <SCard class="min-h-0 overflow-hidden" split>
        <template #header>
          <div class="flex w-full items-center justify-between gap-2">
            <div class="flex items-center gap-2">
              <span class="text-sm font-semibold">对话</span>
              <span class="rounded-full bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
                {{ peerRows.length }}
              </span>
            </div>
            <SButton
              variant="ghost"
              size="sm"
              shape="square"
              class="size-8"
              :disabled="deviceStore.isDiscovering"
              title="刷新设备"
              aria-label="刷新设备"
              @click="refreshDevices"
            >
              <SIcon
                icon="lucide:refresh-cw"
                :class="deviceStore.isDiscovering ? 'animate-spin' : ''"
              />
            </SButton>
          </div>
        </template>

        <div v-if="peerRows.length" class="max-h-72 space-y-1.5 overflow-y-auto md:max-h-[30rem]">
          <button
            v-for="peer in peerRows"
            :key="peer.key"
            type="button"
            class="flex w-full min-w-0 items-center gap-2.5 rounded-xl border px-2.5 py-2.5 text-left transition-colors"
            :class="
              selectedPeer?.key === peer.key
                ? 'border-primary/30 bg-primary/8'
                : 'border-transparent hover:border-border/70 hover:bg-muted/50'
            "
            @click="selectedPeerKey = peer.key"
          >
            <span
              class="flex size-9 shrink-0 items-center justify-center rounded-xl"
              :class="peer.online ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground'"
            >
              <SIcon icon="lucide:monitor-smartphone" class="text-sm" />
            </span>
            <span class="min-w-0 flex-1">
              <span class="flex items-center gap-1.5">
                <span class="min-w-0 truncate text-xs font-semibold">{{ peer.name }}</span>
                <span
                  class="size-1.5 shrink-0 rounded-full"
                  :class="peer.online ? 'bg-success' : 'bg-muted-foreground/40'"
                  :title="peer.online ? '在线' : '离线'"
                />
              </span>
              <span class="mt-0.5 block truncate text-[10px] text-muted-foreground">
                {{ peer.latestText }}
              </span>
            </span>
            <span v-if="peer.latestAt" class="shrink-0 text-[9px] text-muted-foreground">
              {{ formatMessageTime(peer.latestAt) }}
            </span>
          </button>
        </div>
        <div v-else class="flex min-h-36 flex-col items-center justify-center px-3 text-center">
          <div
            class="flex size-11 items-center justify-center rounded-2xl bg-muted text-muted-foreground"
          >
            <SIcon icon="lucide:messages-square" class="text-xl" />
          </div>
          <p class="mt-3 text-xs font-medium">还没有可用对话</p>
          <p class="mt-1 text-[11px] text-muted-foreground">先添加一个附近设备</p>
          <SButton variant="link" size="xs" class="mt-1 h-auto p-0" @click="openDevices">
            去附近设备添加
          </SButton>
        </div>

        <template #footer>
          <div class="flex items-center justify-between gap-2 text-[10px] text-muted-foreground">
            <span>{{ onlineCount }} 台设备在线</span>
            <button type="button" class="hover:text-primary" @click="openDevices">管理设备</button>
          </div>
        </template>
      </SCard>

      <SCard class="min-h-[34rem] overflow-hidden" split>
        <template #header>
          <div class="flex w-full min-w-0 items-center gap-3">
            <div
              class="flex size-10 shrink-0 items-center justify-center rounded-xl"
              :class="
                selectedPeer?.online
                  ? 'bg-primary/10 text-primary'
                  : 'bg-muted text-muted-foreground'
              "
            >
              <SIcon icon="lucide:message-circle" />
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2">
                <span class="truncate text-sm font-semibold">
                  {{ selectedPeer?.name || '选择一个设备' }}
                </span>
                <span
                  v-if="selectedPeer"
                  class="inline-flex shrink-0 items-center gap-1 text-[10px]"
                  :class="selectedPeer.online ? 'text-success' : 'text-muted-foreground'"
                >
                  <span
                    class="size-1.5 rounded-full"
                    :class="selectedPeer.online ? 'bg-success' : 'bg-muted-foreground/50'"
                  />
                  {{ selectedPeer.online ? '在线' : '离线' }}
                </span>
              </div>
              <span
                v-if="selectedPeer?.ip"
                class="mt-0.5 block truncate font-mono text-[10px] text-muted-foreground"
              >
                {{ selectedPeer.ip }}:{{ selectedPeer.port }}
              </span>
            </div>
          </div>
        </template>

        <div class="flex min-h-[22rem] flex-1 flex-col bg-muted/10">
          <div class="min-h-0 flex-1 overflow-y-auto p-3 sm:p-5">
            <div v-if="selectedMessages.length" class="space-y-4">
              <div
                v-for="message in selectedMessages"
                :key="message.id"
                class="flex items-end gap-2"
                :class="message.direction === 'send' ? 'justify-end' : 'justify-start'"
              >
                <div
                  v-if="message.direction === 'receive'"
                  class="flex size-7 shrink-0 items-center justify-center rounded-full bg-muted text-muted-foreground"
                >
                  <SIcon icon="lucide:monitor-smartphone" class="text-xs" />
                </div>
                <div class="group max-w-[min(86%,34rem)] min-w-0">
                  <div
                    class="mb-1 flex items-center gap-2 text-[10px] text-muted-foreground"
                    :class="message.direction === 'send' ? 'justify-end' : ''"
                  >
                    <span>{{ message.direction === 'send' ? '我' : selectedPeer?.name }}</span>
                    <span>{{ formatMessageTime(message.createdAt) }}</span>
                  </div>
                  <div
                    class="relative rounded-2xl px-3.5 py-2.5 text-sm shadow-sm"
                    :class="
                      message.direction === 'send'
                        ? 'rounded-br-md bg-primary text-primary-foreground'
                        : 'rounded-bl-md border border-border/70 bg-card'
                    "
                  >
                    <p class="whitespace-pre-wrap break-words [overflow-wrap:anywhere]">
                      {{ message.text }}
                    </p>
                    <button
                      type="button"
                      class="mt-2 inline-flex items-center gap-1 text-[10px] opacity-70 transition-opacity hover:opacity-100"
                      :class="
                        message.direction === 'send'
                          ? 'text-primary-foreground'
                          : 'text-muted-foreground'
                      "
                      title="复制消息"
                      @click="copyText(message.text)"
                    >
                      <SIcon icon="lucide:copy" class="text-[10px]" />
                      复制
                    </button>
                  </div>
                </div>
                <div
                  v-if="message.direction === 'send'"
                  class="flex size-7 shrink-0 items-center justify-center rounded-full bg-primary/10 text-primary"
                >
                  <SIcon icon="lucide:user-round" class="text-xs" />
                </div>
              </div>
            </div>
            <div
              v-else
              class="flex h-full min-h-64 flex-col items-center justify-center text-center"
            >
              <div
                class="flex size-14 items-center justify-center rounded-2xl bg-primary/10 text-primary"
              >
                <SIcon icon="lucide:message-square-text" class="text-2xl" />
              </div>
              <p class="mt-4 text-sm font-semibold">
                {{ selectedPeer ? '开始一段新的对话' : '选择一个设备开始发送消息' }}
              </p>
              <p class="mt-1 max-w-xs text-xs text-muted-foreground">
                {{
                  selectedPeer?.online
                    ? '消息会通过加密连接发送，并保存在本机的消息记录中。'
                    : '设备当前不在线，重新连接后即可继续发送。'
                }}
              </p>
            </div>
          </div>

          <div class="border-t border-border/70 bg-card p-3 sm:p-4">
            <div
              v-if="sendError"
              class="mb-2 flex items-start gap-1.5 text-[11px] text-destructive"
              role="alert"
            >
              <SIcon icon="lucide:circle-alert" class="mt-0.5 shrink-0 text-xs" />
              <span>{{ sendError }}</span>
            </div>
            <div
              class="overflow-hidden rounded-xl border border-border/80 transition-colors focus-within:border-primary/45 focus-within:ring-4 focus-within:ring-primary/10"
              :class="!selectedPeer?.ip || !selectedPeer.online ? 'bg-muted/30' : 'bg-card'"
            >
              <textarea
                v-model="draftText"
                rows="3"
                maxlength="524288"
                :disabled="!selectedPeer?.ip || !selectedPeer.online || isSendingText"
                class="min-h-20 w-full resize-y bg-transparent px-3 py-2.5 text-sm leading-6 outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-70"
                :placeholder="
                  selectedPeer?.online
                    ? '输入消息，按 ⌘/Ctrl ↵ 发送…'
                    : '设备不在线，暂时无法发送消息'
                "
                aria-label="消息内容"
                @keydown="onTextComposerKeydown"
              />
              <div
                class="flex items-center justify-between gap-3 border-t border-border/60 px-3 py-2"
              >
                <span
                  class="text-[10px]"
                  :class="
                    draftTextBytes > maxTextBytes ? 'text-destructive' : 'text-muted-foreground'
                  "
                >
                  {{ draftTextBytes.toLocaleString() }} / 524,288 字节
                </span>
                <div class="flex items-center gap-2">
                  <button
                    v-if="draftText"
                    type="button"
                    class="text-[11px] text-muted-foreground hover:text-foreground"
                    @click="draftText = ''"
                  >
                    清空
                  </button>
                  <SButton size="sm" :disabled="!canSendText" @click="sendTextToSelectedPeer">
                    <SIcon icon="lucide:send" :class="isSendingText ? 'animate-pulse' : ''" />
                    {{ isSendingText ? '发送中…' : '发送' }}
                  </SButton>
                </div>
              </div>
            </div>
          </div>
        </div>
      </SCard>
    </div>

    <div class="flex items-center gap-2 text-[11px] text-muted-foreground">
      <SIcon icon="lucide:shield-check" class="shrink-0 text-success" />
      <span>文字消息通过加密连接传输，最多支持 512 KB。</span>
      <button
        type="button"
        class="ml-auto hover:text-primary"
        @click="transferStore.clearTextMessages()"
      >
        清空全部消息
      </button>
    </div>
  </div>
</template>
