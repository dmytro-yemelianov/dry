<script setup lang="ts">
import { computed } from 'vue';
import authorSource from '../../examples/author.ts?raw';
import generativeSource from '../../examples/generative.ts?raw';
import lowerSource from '../../examples/lower.ts?raw';
import optimizeSource from '../../examples/optimize.ts?raw';
import simulateSource from '../../examples/simulate.ts?raw';
import verifySource from '../../examples/verify.ts?raw';

const props = defineProps<{ src?: string; code?: string; outputs?: string[] }>();
const documentedExamples: Record<string, string> = {
  author: authorSource,
  generative: generativeSource,
  lower: lowerSource,
  optimize: optimizeSource,
  simulate: simulateSource,
  verify: verifySource,
};
const preview = computed(() =>
  props.src && documentedExamples[props.src] ? `/reference/previews/${props.src}.svg` : undefined,
);
const documentedSource = computed(() => (props.src ? documentedExamples[props.src] : undefined));
</script>

<template>
  <aside class="public-example">
    <strong>{{ src ? `Dry example: ${src}` : 'Dry example' }}</strong>
    <p>
      This lightweight docs-only build shows a static preview.
      Interactive execution is available in the public
      <a href="https://dry-public-docs.pages.dev/gallery/">Dry gallery</a>.
    </p>
    <pre v-if="code"><code>{{ code }}</code></pre>
    <details v-else-if="$slots.default">
      <summary>View documented source</summary>
      <pre><slot /></pre>
    </details>
    <details v-else-if="documentedSource">
      <summary>View documented TypeScript source</summary>
      <pre><code>{{ documentedSource }}</code></pre>
    </details>
    <img
      v-if="preview"
      :src="preview"
      :alt="`Static output preview for the ${src} example`"
      loading="lazy"
    />
    <p v-if="outputs?.length">
      Documented outputs: <code>{{ outputs.join(', ') }}</code>
    </p>
  </aside>
</template>
