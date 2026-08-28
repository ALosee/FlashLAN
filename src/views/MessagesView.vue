<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { STextarea } from '@soybeanjs/ui'
import type { MenuOptionData, MenuUi } from '@soybeanjs/headless'
import { SButton } from '@/ui/components/button'
import { SButtonIcon } from '@/ui/components/button'
import { SIcon } from '@/ui/components/icon'
import { useDeviceStore } from '@/stores/device'
import { normalizePeerAddress, type TextMessageItem, useTransferStore } from '@/stores/transfer'

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
const messageScrollEl = ref<HTMLElement | null>(null)
const draftText = ref('')
const isSendingText = ref(false)
const sendError = ref('')
const maxTextBytes = 512 * 1024
const showMessageActions = ref(false)

type MessageAction = 'clear-current' | 'clear-all'

const messageActions: MenuOptionData<MessageAction>[] = [
  {
    label: '清空当前设备消息',
    value: 'clear-current',
    icon: 'lucide:trash-2',
  },
  {
    label: '清空全部消息记录',
    value: 'clear-all',
    icon: 'lucide:messages-square',
  },
]

const messageMenuUi: Partial<MenuUi> = {
  popup: 'w-40 max-w-[calc(100vw-1.5rem)] rounded-xl border border-border bg-card p-1 shadow-lg',
  item: 'min-h-10 rounded-lg text-destructive hover:bg-destructive/8',
  itemIcon: 'size-3.5 shrink-0 text-destructive',
}

const peerSwitcherUi: Partial<MenuUi> = {
  popup: 'w-48 max-w-[calc(100vw-2rem)] rounded-xl border border-border bg-card p-1 shadow-lg',
  item: 'min-h-10 rounded-lg',
  itemIcon: 'size-3.5 shrink-0 text-primary',
}

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
  const historyPeers = Array.from(
    new Set(transferStore.textMessages.map(message => normalizePeerAddress(message.peer))),
  )

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

function getPeerIdentifiers(peer: MessagePeer) {
  return [peer.ip, peer.name, peer.sourceName].filter((value): value is string => Boolean(value))
}

function matchesPeer(message: TextMessageItem, peer: MessagePeer) {
  const messagePeer = normalizePeerAddress(message.peer)
  return getPeerIdentifiers(peer).some(candidate => normalizePeerAddress(candidate) === messagePeer)
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

const peerSwitchOptions = computed<MenuOptionData<string>[]>(() =>
  peerRows.value.map(peer => ({
    label: peer.online ? peer.name : `${peer.name}（离线）`,
    value: peer.key,
    icon: 'lucide:monitor-smartphone',
  })),
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
  void scrollMessagesToBottom('auto')
})

async function scrollMessagesToBottom(behavior: ScrollBehavior = 'smooth') {
  await nextTick()
  const container = messageScrollEl.value
  if (!container) return
  container.scrollTo({ top: container.scrollHeight, behavior })
}

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

function deleteMessage(messageId: string) {
  transferStore.removeTextMessage(messageId)
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
    await scrollMessagesToBottom()
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

function selectMessageAction(item: MenuOptionData<MessageAction>) {
  showMessageActions.value = false
  if (item.value === 'clear-all') {
    transferStore.clearTextMessages()
    return
  }

  if (item.value === 'clear-current' && selectedPeer.value) {
    transferStore.clearTextMessagesForPeers(getPeerIdentifiers(selectedPeer.value))
  }
}

const showPeerSwitcher = ref(false)

function selectPeer(item: MenuOptionData<string>) {
  selectedPeerKey.value = item.value
  showPeerSwitcher.value = false
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
  <div
    class="flex h-full min-h-0 w-full min-w-0 flex-col gap-2 overflow-hidden p-2 md:gap-3 md:p-4"
  >
    <div
      v-if="deviceStore.error"
      class="flex items-start gap-2 rounded-xl border border-destructive/15 bg-destructive/8 px-4 py-3 text-sm text-destructive"
      role="alert"
    >
      <SIcon icon="lucide:circle-alert" class="mt-0.5 shrink-0" />
      <span>{{ deviceStore.error }}</span>
    </div>

    <div class="flex min-h-0 flex-1 flex-col gap-3 bg-card/30 md:flex-row md:gap-4">
      <aside
        class="hidden max-h-72 shrink-0 flex-col rounded-2xl border border-border bg-card md:flex md:max-h-none md:w-60"
      >
        <div class="flex h-14 shrink-0 items-center justify-between gap-3 px-4">
          <div class="flex items-center gap-2">
            <span class="text-xs font-semibold">设备列表</span>
            <span class="rounded-full bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
              {{ peerRows.length }}
            </span>
          </div>
          <SButton
            variant="ghost"
            size="sm"
            shape="square"
            class="size-8 shrink-0"
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

        <div class="min-h-0 flex-1 overflow-y-auto p-3">
          <div v-if="peerRows.length" class="space-y-1">
            <button
              v-for="peer in peerRows"
              :key="peer.key"
              type="button"
              class="flex w-full min-w-0 items-center gap-2.5 rounded-xl px-2.5 py-2.5 text-left transition-colors"
              :class="
                selectedPeer?.key === peer.key
                  ? 'bg-primary/10 text-foreground ring-1 ring-inset ring-primary/25'
                  : 'text-foreground hover:bg-muted/70'
              "
              @click="selectedPeerKey = peer.key"
            >
              <span
                class="flex size-9 shrink-0 items-center justify-center rounded-xl"
                :class="
                  peer.online ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground'
                "
              >
                <SIcon icon="lucide:monitor-smartphone" class="text-sm" />
              </span>
              <span class="min-w-0 flex-1">
                <span class="flex items-center justify-between gap-2">
                  <span class="flex min-w-0 items-center gap-1.5">
                    <span class="min-w-0 truncate text-xs font-semibold">{{ peer.name }}</span>
                    <span
                      class="size-1.5 shrink-0 rounded-full"
                      :class="peer.online ? 'bg-success' : 'bg-muted-foreground/40'"
                      :title="peer.online ? '在线' : '离线'"
                    />
                  </span>
                  <span v-if="peer.latestAt" class="shrink-0 text-[9px] text-muted-foreground">
                    {{ formatMessageTime(peer.latestAt) }}
                  </span>
                </span>
                <span class="mt-0.5 block truncate text-[10px] text-muted-foreground">
                  {{ peer.latestText }}
                </span>
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
        </div>

        <div
          class="flex shrink-0 items-center justify-between gap-2 px-4 py-3 text-[10px] text-muted-foreground"
        >
          <span>{{ onlineCount }} 台设备在线</span>
          <button type="button" class="hover:text-primary" @click="openDevices">管理设备</button>
        </div>
      </aside>

      <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-background/25">
        <header class="flex h-14 shrink-0 items-center justify-between gap-3">
          <div v-if="selectedPeer" class="md:hidden">
            <SDropdownMenu
              v-model:open="showPeerSwitcher"
              :items="peerSwitchOptions"
              placement="bottom-start"
              :show-arrow="false"
              :ui="peerSwitcherUi"
              @select="selectPeer"
            >
              <template #trigger>
                <SButton
                  variant="ghost"
                  color="secondary"
                  class="min-w-0 max-w-full justify-start gap-2.5 rounded-xl bg-transparent px-0 py-0 text-left data-[normal]:hover:bg-transparent data-[normal]:active:bg-transparent"
                  aria-label="切换设备"
                >
                  <span
                    class="flex size-8 shrink-0 items-center justify-center rounded-lg"
                    :class="
                      selectedPeer.online
                        ? 'bg-primary/10 text-primary'
                        : 'bg-muted text-muted-foreground'
                    "
                  >
                    <SIcon icon="lucide:monitor-smartphone" class="text-sm" />
                  </span>
                  <span class="min-w-0">
                    <span class="flex items-center gap-2">
                      <span class="max-w-48 truncate text-sm font-semibold">
                        {{ selectedPeer.name }}
                      </span>
                      <span
                        class="size-1.5 shrink-0 rounded-full"
                        :class="selectedPeer.online ? 'bg-success' : 'bg-muted-foreground/40'"
                      />
                    </span>
                    <span
                      v-if="selectedPeer.ip"
                      class="block truncate font-mono text-[10px] text-muted-foreground"
                    >
                      {{ selectedPeer.ip }}:{{ selectedPeer.port }}
                    </span>
                  </span>
                  <SIcon
                    icon="lucide:chevron-down"
                    class="shrink-0 text-xs text-muted-foreground"
                  />
                </SButton>
              </template>
            </SDropdownMenu>
          </div>

          <div v-if="selectedPeer" class="hidden min-w-0 items-center gap-2.5 md:flex">
            <span
              class="flex size-8 shrink-0 items-center justify-center rounded-lg"
              :class="
                selectedPeer.online
                  ? 'bg-primary/10 text-primary'
                  : 'bg-muted text-muted-foreground'
              "
            >
              <SIcon icon="lucide:monitor-smartphone" class="text-sm" />
            </span>
            <div class="min-w-0">
              <div class="flex items-center gap-2">
                <span class="truncate text-sm font-semibold">{{ selectedPeer.name }}</span>
                <span
                  class="size-1.5 shrink-0 rounded-full"
                  :class="selectedPeer.online ? 'bg-success' : 'bg-muted-foreground/40'"
                />
                <span class="shrink-0 text-[10px] text-muted-foreground">
                  {{ selectedPeer.online ? '在线' : '离线' }}
                </span>
              </div>
              <p
                v-if="selectedPeer.ip"
                class="truncate font-mono text-[10px] text-muted-foreground"
              >
                {{ selectedPeer.ip }}:{{ selectedPeer.port }}
              </p>
            </div>
          </div>
          <span v-else class="text-sm text-muted-foreground">选择一个设备开始对话</span>

          <SDropdownMenu
            v-model:open="showMessageActions"
            :items="messageActions"
            placement="bottom-end"
            :show-arrow="false"
            :ui="messageMenuUi"
            @select="selectMessageAction"
          >
            <template #trigger>
              <SButtonIcon
                color="primary"
                variant="soft"
                icon="lucide:ellipsis"
                aria-label="更多操作"
                title="更多操作"
                class="size-9 rounded-xl"
              />
            </template>
          </SDropdownMenu>
        </header>

        <div ref="messageScrollEl" class="min-h-0 flex-1 overflow-y-auto py-2">
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
              <div class="group relative max-w-[min(86%,34rem)] min-w-0">
                <div
                  class="mb-1 flex items-center gap-2 text-[10px] text-muted-foreground"
                  :class="message.direction === 'send' ? 'justify-end' : ''"
                >
                  <span>{{ message.direction === 'send' ? '我' : selectedPeer?.name }}</span>
                  <span>{{ formatMessageTime(message.createdAt) }}</span>
                </div>
                <div
                  class="rounded-2xl px-3.5 py-2.5 text-sm shadow-sm"
                  :class="
                    message.direction === 'send'
                      ? 'rounded-br-md bg-primary text-primary-foreground'
                      : 'rounded-bl-md bg-card ring-1 ring-inset ring-border/60'
                  "
                >
                  <p class="whitespace-pre-wrap break-words [overflow-wrap:anywhere]">
                    {{ message.text }}
                  </p>
                </div>
                <div
                  class="absolute bottom-0 z-10 flex items-center gap-1 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100"
                  :class="message.direction === 'send' ? 'right-full pr-2' : 'left-full pl-2'"
                >
                  <SButtonIcon
                    icon="lucide:copy"
                    color="secondary"
                    variant="ghost"
                    icon-class="text-xs"
                    class="size-7 rounded-lg hover:bg-muted hover:text-foreground"
                    title="复制消息"
                    aria-label="复制消息"
                    @click="copyText(message.text)"
                  />
                  <SButtonIcon
                    icon="lucide:trash-2"
                    color="destructive"
                    variant="ghost"
                    icon-class="text-xs"
                    class="size-7 rounded-lg hover:bg-destructive/10 hover:text-destructive"
                    title="删除消息"
                    aria-label="删除消息"
                    @click="deleteMessage(message.id)"
                  />
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
          <div v-else class="flex h-full min-h-64 flex-col items-center justify-center text-center">
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

        <div class="">
          <div
            v-if="sendError"
            class="mb-2 flex items-start gap-1.5 rounded-lg bg-destructive/8 px-3 py-2 text-[11px] text-destructive"
            role="alert"
          >
            <SIcon icon="lucide:circle-alert" class="mt-0.5 shrink-0 text-xs" />
            <span>{{ sendError }}</span>
          </div>
          <STextarea
            v-model="draftText"
            size="lg"
            :autosize="{ minRows: 3, maxRows: 8 }"
            :maxlength="maxTextBytes"
            :disabled="!selectedPeer?.ip || !selectedPeer.online || isSendingText"
            :placeholder="
              selectedPeer?.online ? '输入消息，按 ⌘/Ctrl ↵ 发送…' : '设备不在线，暂时无法发送消息'
            "
            aria-label="消息内容"
            :ui="{
              root: 'rounded-2xl border-border bg-background/60 shadow-none transition-colors focus-within:ring-4 focus-within:ring-primary/10',
              control:
                'min-h-24 px-4 py-3 text-sm leading-6 placeholder:text-muted-foreground disabled:opacity-70',
            }"
            @keydown="onTextComposerKeydown"
          >
            <template #footer>
              <div class="flex items-center justify-between gap-3 px-4 py-2.5">
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
            </template>
          </STextarea>
        </div>
      </section>
    </div>
  </div>
</template>
