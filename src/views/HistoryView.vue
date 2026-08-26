<script setup lang="ts">
import { computed } from 'vue'
import { SIcon } from '@/ui/components/icon'
import { useTransferStore } from '@/stores/transfer'

const transferStore = useTransferStore()
const records = computed(() =>
  transferStore.tasks.filter(task => task.status === 'completed' || task.status === 'failed'),
)

function formatBytes(bytes: number) {
  if (!bytes) return '未知大小'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`
}

function recordStatus(status: string) {
  return status === 'completed' ? '已完成' : '失败'
}
</script>

<template>
  <div class="mx-auto flex w-full max-w-5xl flex-col gap-6 p-4 md:p-8">
    <div class="flex items-end justify-between gap-4">
      <div>
        <h1 class="text-2xl font-bold tracking-tight">传输记录</h1>
        <p class="mt-1 text-sm text-muted-foreground">查看历史收发文件</p>
      </div>
      <span
        v-if="records.length"
        class="shrink-0 rounded-full bg-muted/70 px-2.5 py-1 text-[11px] text-muted-foreground"
      >
        {{ records.length }} 条记录
      </span>
    </div>

    <div
      v-if="records.length"
      class="overflow-hidden rounded-2xl border border-border/80 dark:border-border/10 bg-card shadow-sm"
    >
      <div class="divide-y divide-border/70 dark:divide-border/10">
        <div
          v-for="item in records"
          :key="item.id"
          class="group grid grid-cols-[2.75rem_minmax(0,1fr)_auto] items-center gap-3 px-3 py-3.5 transition-colors hover:bg-muted/30 sm:gap-4 sm:px-4"
        >
          <div
            class="flex size-11 items-center justify-center rounded-2xl"
            :class="
              item.status === 'completed'
                ? 'bg-success/10 text-success'
                : 'bg-destructive/10 text-destructive'
            "
          >
            <SIcon
              :icon="item.direction === 'receive' ? 'lucide:download' : 'lucide:upload'"
              class="text-lg"
            />
          </div>
          <div class="min-w-0">
            <div class="flex min-w-0 items-center gap-2 leading-5">
              <div class="min-w-0 truncate text-sm font-semibold leading-5">
                {{ item.fileName }}
              </div>
              <span class="shrink-0 whitespace-nowrap text-xs leading-5 text-muted-foreground">
                · {{ formatBytes(item.total) }}
              </span>
            </div>
            <div
              class="mt-1 flex min-w-0 items-center gap-1.5 truncate text-xs text-muted-foreground"
            >
              <SIcon icon="lucide:clock-3" class="shrink-0 text-xs" />
              <span class="truncate">
                {{ item.direction === 'receive' ? '接收自' : '发送至' }} {{ item.targetIp }}
              </span>
            </div>
            <div
              v-if="item.direction === 'receive'"
              class="mt-1 flex min-w-0 items-center gap-1 truncate text-[11px] text-success"
            >
              <SIcon icon="lucide:folder-check" class="shrink-0 text-xs" />
              <span class="truncate">已保存至 {{ item.filePath || 'Download/FlashLAN' }}</span>
            </div>
          </div>
          <div
            class="mt-1 flex shrink-0 items-center gap-1.5 self-start text-xs font-semibold"
            :class="item.status === 'completed' ? 'text-success' : 'text-destructive'"
          >
            <SIcon
              :icon="item.status === 'completed' ? 'lucide:check' : 'lucide:x'"
              class="text-sm"
            />
            <span>{{ recordStatus(item.status) }}</span>
            <span
              class="size-1.5 rounded-full"
              :class="item.status === 'completed' ? 'bg-success' : 'bg-destructive'"
            />
          </div>
        </div>
      </div>
    </div>

    <div
      v-else
      class="rounded-2xl border border-dashed border-border/80 dark:border-border/10 bg-card/60 p-10 text-center"
    >
      <div
        class="mx-auto flex size-14 items-center justify-center rounded-2xl bg-muted text-muted-foreground"
      >
        <SIcon icon="lucide:history" class="text-2xl" />
      </div>
      <div class="mt-4 text-sm font-semibold">暂无传输记录</div>
      <div class="mt-1 text-xs text-muted-foreground">完成文件收发后，记录会显示在这里</div>
    </div>
  </div>
</template>
