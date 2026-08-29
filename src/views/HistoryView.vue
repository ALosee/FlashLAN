<script setup lang="ts">
import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { SDropdownMenu } from '@soybeanjs/ui'
import type { MenuOptionData, MenuUi } from '@soybeanjs/headless'
import { SButton, SButtonIcon } from '@/ui/components/button'
import { SDialog } from '@/ui/components/dialog'
import { SIcon } from '@/ui/components/icon'
import { EmptyState, PageHeader, StatusIndicator, TransferRow } from '@/ui/patterns'
import { type TransferTask, useTransferStore } from '@/stores/transfer'
import { isTauri } from '@/utils/tauri'

const transferStore = useTransferStore()
const showClearConfirm = ref(false)
const records = computed(() =>
  transferStore.tasks
    .filter(task => task.status === 'completed' || task.status === 'failed')
    .slice()
    .sort((a, b) => (b.createdAt ?? 0) - (a.createdAt ?? 0)),
)
const completedCount = computed(
  () => records.value.filter(item => item.status === 'completed').length,
)
const failedCount = computed(() => records.value.filter(item => item.status === 'failed').length)
const openingRecordId = ref('')
const feedback = ref('')
const feedbackIsError = ref(false)
const isAndroid = typeof navigator !== 'undefined' && /Android/i.test(navigator.userAgent)

type RecordAction = 'delete'

const recordActions: MenuOptionData<RecordAction>[] = [
  {
    label: '删除记录',
    value: 'delete',
    icon: 'lucide:trash-2',
  },
]

const recordMenuUi: Partial<MenuUi> = {
  popup: 'w-32 rounded-lg border border-border bg-card p-1 shadow-lg',
  item: 'min-h-11 rounded-lg sm:min-h-8',
  itemIcon: 'size-3.5 shrink-0 text-destructive',
}

function recordOpenPath(item: TransferTask) {
  return item.fileOpenPath || item.filePath
}

function recordPathLabel(item: TransferTask) {
  if (isAndroid && item.direction === 'send') return '来自手机文件'
  return item.filePath
}

function canOpenRecord(item: TransferTask) {
  const path = recordOpenPath(item)
  if (!path) return false
  return !isAndroid || (item.direction === 'receive' && path.startsWith('content://'))
}

function formatBytes(bytes: number) {
  if (!bytes) return '未知大小'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`
}

function recordStatus(status: TransferTask['status']) {
  return status === 'completed' ? '已完成' : '失败'
}

function formatTime(createdAt?: number) {
  if (!createdAt) return ''
  try {
    return new Intl.DateTimeFormat('zh-CN', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    }).format(new Date(createdAt))
  } catch {
    return new Date(createdAt).toLocaleString()
  }
}

function clearFeedback() {
  feedback.value = ''
  feedbackIsError.value = false
}

async function openRecordLocation(item: TransferTask) {
  clearFeedback()
  const path = recordOpenPath(item)
  if (!path) {
    feedbackIsError.value = true
    feedback.value = '这条记录没有可用的本地文件路径'
    return
  }
  if (isAndroid && !canOpenRecord(item)) {
    feedbackIsError.value = true
    feedback.value = '这条 Android 记录没有可用的文件 URI'
    return
  }
  if (!isTauri()) {
    feedbackIsError.value = true
    feedback.value = '浏览器预览无法打开本机目录，请在桌面应用中使用此功能'
    return
  }

  openingRecordId.value = item.id
  try {
    await invoke('open_file_location', { path, fileName: item.fileName })
  } catch (error) {
    feedbackIsError.value = true
    feedback.value = String(error).replace(/^Error:\s*/, '') || '无法打开文件所在目录'
  } finally {
    openingRecordId.value = ''
  }
}

function removeRecord(item: TransferTask) {
  transferStore.removeTask(item.id)
  feedbackIsError.value = false
  feedback.value = `已删除「${item.fileName}」的传输记录`
}

function selectRecordAction(action: MenuOptionData<RecordAction>, item: TransferTask) {
  if (action.value === 'delete') removeRecord(item)
}

function confirmClearHistory() {
  showClearConfirm.value = true
}

function doClearHistory() {
  transferStore.clearHistory()
  showClearConfirm.value = false
  feedbackIsError.value = false
  feedback.value = '传输记录已清空，文件本身未被删除'
}
</script>

<template>
  <div class="fl-page fl-content-list flex flex-col gap-6">
    <PageHeader
      title="传输记录"
      description="完整查看历史收发文件，删除记录不会影响文件本身"
      mobile-actions-inline
    >
      <template #after-title>
        <span
          v-if="records.length"
          class="rounded-md bg-muted px-2 py-1 text-xs font-medium text-muted-foreground"
        >
          {{ records.length }} 条
        </span>
        <SButtonIcon
          v-if="records.length"
          color="destructive"
          variant="ghost"
          icon="lucide:trash-2"
          class="ml-auto size-11 rounded-lg md:hidden"
          title="清空传输记录"
          aria-label="清空传输记录"
          @click="confirmClearHistory"
        />
      </template>

      <template v-if="records.length" #status>
        <StatusIndicator :label="completedCount + ' 已完成'" tone="success" />
        <StatusIndicator v-if="failedCount" :label="failedCount + ' 失败'" tone="destructive" />
      </template>

      <template #actions>
        <SButton
          v-if="records.length"
          variant="outline"
          size="sm"
          class="shrink-0"
          @click="confirmClearHistory"
        >
          <SIcon icon="lucide:trash-2" />
          清空记录
        </SButton>
      </template>
    </PageHeader>

    <!--
 SDialog 会把未传的布尔 prop 转发为 false，吞掉库内默认值，
         因此 show-confirm / show-cancel 必须显式给出 
-->
    <SDialog
      v-model:open="showClearConfirm"
      title="清空传输记录"
      description="确定清空全部传输记录吗？文件本身不会被删除。"
      size="sm"
      is-alert
      alert-type="warning"
      :show-confirm="true"
      :show-cancel="true"
      confirm-text="清空"
      cancel-text="取消"
      @confirm="doClearHistory"
      @cancel="showClearConfirm = false"
    />

    <Transition name="fl-state">
      <div
        v-if="feedback"
        class="flex items-start gap-2 rounded-xl border px-3 py-2 text-xs"
        :class="
          feedbackIsError
            ? 'border-destructive/15 bg-destructive/8 text-destructive'
            : 'border-success/15 bg-success/8 text-success'
        "
        role="status"
      >
        <SIcon
          :icon="feedbackIsError ? 'lucide:circle-alert' : 'lucide:circle-check'"
          class="mt-1 shrink-0"
        />
        <span>{{ feedback }}</span>
      </div>
    </Transition>

    <TransitionGroup
      v-if="records.length"
      name="fl-list"
      tag="div"
      class="divide-y divide-border overflow-hidden rounded-lg border border-border bg-card"
    >
      <TransferRow
        v-for="item in records"
        :key="item.id"
        :direction="item.direction"
        :file-name="item.fileName"
        :peer="item.targetIp"
        :size="formatBytes(item.total)"
        :time="formatTime(item.createdAt)"
        :status="recordStatus(item.status)"
        :tone="item.status === 'completed' ? 'success' : 'destructive'"
        :path="recordPathLabel(item) || '暂无本地文件路径'"
        :path-title="
          isAndroid && item.direction === 'send' ? '源文件来自手机文件选择器' : item.filePath
        "
        :error="item.error"
      >
        <template #actions>
          <SButton
            v-if="canOpenRecord(item)"
            variant="soft"
            color="primary"
            size="sm"
            class="min-h-11 sm:min-h-8"
            :disabled="openingRecordId === item.id"
            :title="isAndroid ? '交给 Android 文件应用打开' : '打开文件所在目录'"
            @click="openRecordLocation(item)"
          >
            <SIcon
              :icon="openingRecordId === item.id ? 'lucide:loader-circle' : 'lucide:folder-open'"
              :class="openingRecordId === item.id ? 'animate-spin' : ''"
            />
            {{ isAndroid ? '打开文件' : '打开目录' }}
          </SButton>
          <span
            v-else-if="isAndroid && item.direction === 'receive' && item.filePath"
            class="text-xs text-muted-foreground"
            title="旧记录缺少 Android 文件 URI"
          >
            已保存
          </span>
          <SDropdownMenu
            :items="recordActions"
            placement="bottom-end"
            :show-arrow="false"
            :ui="recordMenuUi"
            @select="action => selectRecordAction(action, item)"
          >
            <template #trigger>
              <SButtonIcon
                icon="lucide:ellipsis"
                variant="ghost"
                class="size-11 sm:size-8"
                :aria-label="`管理记录：${item.fileName}`"
                title="更多记录操作"
              />
            </template>
          </SDropdownMenu>
        </template>
      </TransferRow>
    </TransitionGroup>

    <EmptyState
      v-else
      icon="lucide:history"
      title="暂无传输记录"
      description="完成文件收发后，记录会显示在这里"
    />
  </div>
</template>
