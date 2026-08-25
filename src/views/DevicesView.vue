<script setup lang="ts">
import { onMounted } from 'vue'
import { SCard } from '@/ui/components/card'
import { SButton } from '@/ui/components/button'
import { SIcon } from '@/ui/components/icon'
import { SBadge } from '@/ui/components/badge'
import { useDeviceStore } from '@/stores/device'

const deviceStore = useDeviceStore()

onMounted(() => {
  deviceStore.discover()
})
</script>

<template>
  <div class="p-4 md:p-6 max-w-5xl mx-auto w-full space-y-4">
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
      <div>
        <h1 class="text-xl font-bold tracking-tight">附近设备</h1>
        <p class="text-sm text-muted-foreground mt-1">基于 mDNS 自动发现，需在同一局域网</p>
      </div>
      <SButton :disabled="deviceStore.isDiscovering" @click="deviceStore.discover()">
        <SIcon icon="lucide:refresh-cw" :class="deviceStore.isDiscovering ? 'animate-spin' : ''" />
        {{ deviceStore.isDiscovering ? '扫描中...' : '刷新' }}
      </SButton>
    </div>

    <div
      v-if="deviceStore.error"
      class="rounded-lg bg-destructive/10 text-destructive text-sm px-4 py-3"
    >
      {{ deviceStore.error }}
    </div>

    <div
      v-if="deviceStore.devices.length === 0 && !deviceStore.isDiscovering"
      class="rounded-xl border border-dashed p-8 text-center"
    >
      <div class="size-12 rounded-full bg-muted flex items-center justify-center mx-auto">
        <SIcon icon="lucide:scan-search" class="text-xl text-muted-foreground" />
      </div>
      <div class="text-sm font-medium mt-3">未发现设备</div>
      <div class="text-xs text-muted-foreground mt-1">
        请确保另一台设备已启动 FlashLAN 并在同一 WiFi
      </div>
    </div>

    <div v-else class="grid grid-cols-1 sm:grid-cols-2 gap-3">
      <SCard v-for="d in deviceStore.devices" :key="d.id" class="p-4!">
        <div class="flex items-center gap-3">
          <div class="size-10 rounded-xl bg-muted flex items-center justify-center shrink-0">
            <SIcon
              :icon="
                d.platform === 'windows'
                  ? 'lucide:monitor'
                  : d.platform === 'macos'
                    ? 'lucide:laptop'
                    : 'lucide:smartphone'
              "
            />
          </div>
          <div class="flex-1 min-w-0">
            <div class="text-sm font-medium truncate">{{ d.name }}</div>
            <div class="text-xs text-muted-foreground font-mono truncate">
              {{ d.ip }}:{{ d.port }}
            </div>
            <div class="text-xs text-muted-foreground truncate">{{ d.platform }}</div>
          </div>
          <SBadge color="success" size="sm">在线</SBadge>
        </div>
      </SCard>
    </div>

    <SCard v-if="deviceStore.localDevice">
      <template #header>
        <span class="text-sm font-medium flex items-center gap-2">
          <SIcon icon="lucide:monitor" />
          本机
        </span>
      </template>
      <div class="flex items-center gap-3">
        <div class="size-9 rounded-xl bg-primary/10 flex items-center justify-center">
          <SIcon icon="lucide:laptop" class="text-primary" />
        </div>
        <div>
          <div class="text-sm font-medium">{{ deviceStore.localDevice.name }}</div>
          <div class="text-xs text-muted-foreground font-mono">
            {{ deviceStore.localDevice.ip }}:{{ deviceStore.localDevice.port }} ·
            {{ deviceStore.localDevice.platform }}
          </div>
        </div>
      </div>
    </SCard>
  </div>
</template>
