<script setup>
/**
 * A titled group of controls.
 *
 * `inert` means **this architecture's code never reads these**, not "the
 * current mode happens to have them at zero". The difference matters: Bloom
 * on Concert Hall is at zero and works the moment it is turned up, so dimming
 * it would hide a control that does something. The spring's tension on a
 * plate is read by nothing at all, and saying so is the honest thing.
 *
 * An inert panel is dimmed **and unreachable**. Leaving the knobs turnable
 * was worse than either: a control that moves and changes nothing is a
 * control that has lied, and somebody who turns it has to work out for
 * themselves that it did not matter. The reason is written in the header
 * instead, which is the part they actually need.
 */
defineProps({
  title: { type: String, required: true },
  hint: { type: String, default: '' },
  inert: { type: Boolean, default: false },
  inertReason: { type: String, default: '' },
});
</script>

<template>
  <section
    class="rounded-xl border border-[var(--line)] bg-[var(--panel)] p-3 transition-opacity"
    :class="inert ? 'opacity-45' : ''"
  >
    <header class="mb-2 flex items-baseline gap-2">
      <h2 class="text-[10px] font-semibold uppercase tracking-[0.08em] text-[var(--dim)]">
        {{ title }}
      </h2>
      <span v-if="inert && inertReason" class="text-[10px] italic text-[var(--faint)]">
        {{ inertReason }}
      </span>
      <span v-else-if="hint" class="text-[10px] text-[var(--faint)]">{{ hint }}</span>
    </header>
    <!-- `pointer-events-none` rather than a `disabled` on each control:
         it cannot be forgotten when a control is added later. -->
    <div :class="inert ? 'pointer-events-none select-none' : ''" :aria-disabled="inert">
      <slot />
    </div>
  </section>
</template>
