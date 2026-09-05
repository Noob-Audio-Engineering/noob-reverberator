<script setup>
/** A titled group of controls. Nothing but a frame. */
defineProps({ title: { type: String, required: true }, hint: { type: String, default: '' } });
</script>

<template>
  <section class="rounded-xl border border-[var(--line)] bg-[var(--panel)] p-3">
    <header class="mb-2 flex items-baseline gap-2">
      <h2 class="text-[10px] font-semibold uppercase tracking-[0.08em] text-[var(--dim)]">
        {{ title }}
      </h2>
      <span v-if="hint" class="text-[10px] text-[var(--faint)]">{{ hint }}</span>
    </header>
    <slot />
  </section>
</template>
