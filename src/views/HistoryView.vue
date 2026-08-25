<script setup lang="ts">
import { SCard } from '@/ui/components/card'
import { SIcon } from '@/ui/components/icon'
import { SBadge } from '@/ui/components/badge'

const records = [
  {
    name: 'design-spec-2026.pdf',
    size: '24.5 MB',
    time: '昨天 14:32',
    from: 'MacBook Air',
    status: '已完成',
    icon: 'lucide:file-text',
  },
  {
    name: 'project-assets.zip',
    size: '128.3 MB',
    time: '昨天 10:15',
    from: 'Windows PC',
    status: '已完成',
    icon: 'lucide:file-archive',
  },
  {
    name: 'demo-video.mp4',
    size: '312.0 MB',
    time: '前天 18:20',
    from: 'iPhone 15',
    status: '已完成',
    icon: 'lucide:file-video',
  },
]
</script>

<template>
  <div class="p-4 md:p-6 max-w-5xl mx-auto w-full space-y-4">
    <div>
      <h1 class="text-xl font-bold tracking-tight">传输记录</h1>
      <p class="text-sm text-muted-foreground mt-1">查看历史收发文件</p>
    </div>
    <SCard>
      <div class="divide-y -m-6">
        <div
          v-for="item in records"
          :key="item.name"
          class="flex items-center gap-4 p-4 hover:bg-muted/30 transition-colors"
        >
          <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
            <SIcon :icon="item.icon" />
          </div>
          <div class="flex-1 min-w-0">
            <div class="text-sm font-medium truncate flex items-center gap-2">
              {{ item.name }}
              <span class="text-xs text-muted-foreground font-normal">· {{ item.size }}</span>
            </div>
            <div class="text-xs text-muted-foreground flex items-center gap-1.5 mt-1 truncate">
              <SIcon icon="lucide:clock-3" class="text-xs" />
              {{ item.time }} · 接收自 {{ item.from }} · 已保存至 ~/Downloads
            </div>
          </div>
          <SBadge color="success" size="sm" class="shrink-0">
            <SIcon icon="lucide:check" class="text-xs" />
            {{ item.status }}
          </SBadge>
        </div>
      </div>
    </SCard>
  </div>
</template>
