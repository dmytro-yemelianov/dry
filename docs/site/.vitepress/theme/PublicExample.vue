<script setup lang="ts">
defineProps<{ src?: string; code?: string; outputs?: string[] }>();
</script>

<template>
  <aside class="public-example">
    <strong>Licensed product example</strong>
    <p>
      Interactive execution is available in the authenticated Dry product.
      The public documentation does not ship the SDK or WebAssembly engine.
    </p>
    <pre v-if="code"><code>{{ code }}</code></pre>
    <details v-else-if="$slots.default">
      <summary>View documented source</summary>
      <pre><slot /></pre>
    </details>
  </aside>
</template>
