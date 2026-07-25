<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{ src?: string; code?: string; outputs?: string[] }>();
const documentedExamples = new Set([
  'author',
  'generative',
  'lower',
  'optimize',
  'simulate',
  'verify',
]);
const preview = computed(() =>
  props.src && documentedExamples.has(props.src) ? `/reference/previews/${props.src}.svg` : undefined,
);
</script>

<template>
  <aside class="public-example">
    <strong>{{ src ? `Licensed product example: ${src}` : 'Licensed product example' }}</strong>
    <p>
      Interactive execution is available in the authenticated Dry product.
      The public documentation does not ship the SDK or WebAssembly engine.
    </p>
    <img
      v-if="preview"
      :src="preview"
      :alt="`Static output preview for the ${src} example`"
      loading="lazy"
    />
    <p v-if="outputs?.length">
      Documented outputs: <code>{{ outputs.join(', ') }}</code>
    </p>
    <pre v-if="code"><code>{{ code }}</code></pre>
    <details v-else-if="$slots.default">
      <summary>View documented source</summary>
      <pre><slot /></pre>
    </details>
  </aside>
</template>
