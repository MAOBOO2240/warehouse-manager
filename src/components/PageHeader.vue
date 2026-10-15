<script setup>
defineProps({
  title: { type: String, required: true },
  hint: { type: String, default: '' },
})
</script>

<template>
  <header class="mb-6 flex items-end justify-between gap-4">
    <div>
      <h1 class="text-[26px] font-bold tracking-wide">{{ title }}</h1>
      <p v-if="hint" class="mt-1 text-[13.5px] text-mute">{{ hint }}</p>
    </div>
    <div class="flex shrink-0 gap-2.5">
      <slot />
    </div>
  </header>
</template>
