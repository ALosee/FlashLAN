<script setup lang="ts">
import { onMounted } from 'vue'
import { SCard } from '@/ui/components/card'
import { SButton } from '@/ui/components/button'
import { SIcon } from '@/ui/components/icon'
import { useDeviceStore } from '@/stores/device'

const deviceStore = useDeviceStore()

function platformLabel(platform: string) {
  if (platform === 'macos') return 'macOS'
  if (platform === 'windows') return 'Windows'
  if (platform === 'android') return 'Android'
  if (platform === 'ios') return 'iPhone / iPad'
  if (platform === 'manual') return '手动添加'
  return platform
}

onMounted(() => {
  deviceStore.discover()
})
</script>

<template>
  <div class="mx-auto flex w-full max-w-5xl flex-col gap-6 p-4 md:p-8">
    <div class="flex flex-col justify-between gap-4 sm:flex-row sm:items-start">
      <div>
        <h1 class="text-2xl font-bold tracking-tight">附近设备</h1>
        <p class="mt-1 text-sm text-muted-foreground">基于 mDNS 自动发现，需在同一局域网</p>
        <div class="mt-3 flex items-center gap-2 text-[11px] text-muted-foreground">
          <span class="size-1.5 rounded-full bg-success" />
          <span>设备发现已开启</span>
          <span class="text-border">·</span>
          <span>{{ deviceStore.devices.length }} 台设备在线</span>
        </div>
      </div>
      <SButton
        class="shrink-0 shadow-sm"
        :disabled="deviceStore.isDiscovering"
        @click="deviceStore.discover()"
      >
        <SIcon icon="lucide:refresh-cw" :class="deviceStore.isDiscovering ? 'animate-spin' : ''" />
        {{ deviceStore.isDiscovering ? '扫描中...' : '刷新' }}
      </SButton>
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
            class="inline-flex shrink-0 items-center gap-1.5 rounded-full bg-success/10 px-2 py-1 text-[10px] font-medium text-success"
          >
            <span class="size-1.5 rounded-full bg-success" />
            在线
          </span>
        </div>
        <div
          class="mt-3 flex items-center justify-between border-t border-border/70 dark:border-border/10 pt-3"
        >
          <span class="text-[11px] text-muted-foreground">局域网设备</span>
          <span class="flex items-center gap-1 text-[11px] text-success">
            <SIcon icon="lucide:wifi" class="text-xs" />
            连接稳定
          </span>
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
  </div>
</template>
