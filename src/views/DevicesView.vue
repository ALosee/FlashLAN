<script setup lang="ts">
import { SCard } from '@/ui/components/card'
import { SButton } from '@/ui/components/button'
import { SIcon } from '@/ui/components/icon'
import { SBadge } from '@/ui/components/badge'
</script>

<template>
  <div class="p-6 max-w-5xl mx-auto w-full space-y-4">
    <div>
      <h1 class="text-xl font-bold tracking-tight">附近设备</h1>
      <p class="text-sm text-muted-foreground mt-1">基于 mDNS + UDP 广播自动发现，需在同一局域网</p>
    </div>

    <SCard>
      <div class="p-8 text-center space-y-4">
        <div class="size-12 rounded-full bg-muted flex items-center justify-center mx-auto">
          <SIcon icon="lucide:scan-search" class="text-xl text-muted-foreground" />
        </div>
        <div>
          <div class="text-sm font-medium">设备发现由 Rust 后端实现</div>
          <div class="text-xs text-muted-foreground mt-1">
            此处为前端占位，下一步将对接 invoke('discover_devices')
          </div>
        </div>
        <div class="flex items-center justify-center gap-2 pt-2">
          <SButton size="sm">
            <SIcon icon="lucide:refresh-cw" />
            扫描设备
          </SButton>
          <SBadge color="secondary">0 台在线</SBadge>
        </div>
      </div>
    </SCard>
  </div>
</template>
