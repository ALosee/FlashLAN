<script setup lang="ts">
defineOptions({
  name: 'PageHeader',
})

withDefaults(
  defineProps<{
    title: string
    description?: string
    mobileActionsInline?: boolean
  }>(),
  {
    description: '',
    mobileActionsInline: false,
  },
)
</script>

<template>
  <header class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
    <div class="min-w-0 flex-1">
      <div class="flex min-w-0 flex-wrap items-center gap-2">
        <h1 class="text-2xl font-bold tracking-tight">{{ title }}</h1>
        <slot name="after-title" />
      </div>
      <p v-if="description" class="mt-1 text-sm text-muted-foreground">
        {{ description }}
      </p>
      <div v-if="$slots.status" class="mt-2 flex flex-wrap items-center gap-2">
        <slot name="status" />
      </div>
    </div>

    <div
      v-if="$slots.actions"
      class="flex-wrap items-center gap-2 sm:shrink-0 sm:justify-end"
      :class="mobileActionsInline ? 'hidden md:flex' : 'flex'"
    >
      <slot name="actions" />
    </div>
  </header>
</template>
