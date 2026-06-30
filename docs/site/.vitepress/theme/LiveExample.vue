<script setup lang="ts">
import { ref, shallowRef, onMounted, computed } from 'vue';
import { EditorView, basicSetup } from 'codemirror';
import { javascript } from '@codemirror/lang-javascript';
import { EditorState } from '@codemirror/state';
import { getDry, initDryEngine } from './dry-engine';
import { runSnippet } from './run-snippet';
import { drawIr } from './render-ir';
import type { Metrics, Report, Toolpath } from '@sdk/ops';

const EXAMPLES = import.meta.glob('../../examples/*.ts', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;

const props = withDefaults(defineProps<{ src?: string; code?: string; outputs?: string[] }>(), {
  outputs: () => ['gcode', 'ir', 'metrics'],
});

function seed(): string {
  if (props.code) return props.code.trim();
  const hit = Object.entries(EXAMPLES).find(([key]) => key.endsWith(`/${props.src}.ts`));
  return (hit?.[1] ?? `// example '${props.src}' not found`).trim();
}

const source = ref(seed());
const error = ref('');
const tab = ref(props.outputs[0]);
const gcode = ref<string[]>([]);
const irText = ref('');
const metricsText = ref('');
const verifyText = ref('');
const canvas = ref<HTMLCanvasElement | null>(null);
const editorHost = ref<HTMLElement | null>(null);
const ready = ref(false);
const view = shallowRef<EditorView | null>(null);

let timer: ReturnType<typeof setTimeout> | undefined;

const tabs = computed(() => props.outputs);
const wants = (name: string) => props.outputs.includes(name);
const isTestMode = (): boolean => {
  const meta = import.meta as ImportMeta & { env?: { MODE?: string } };
  return meta.env?.MODE === 'test';
};

function schedule(): void {
  clearTimeout(timer);
  timer = setTimeout(runNow, 250);
}

function isToolpath(value: unknown): value is Toolpath {
  return !!value && typeof value === 'object' && Array.isArray((value as Toolpath).segments);
}

function isMetrics(value: unknown): value is Metrics {
  return !!value && typeof value === 'object' && typeof (value as Metrics).total_time_s === 'number';
}

function isReport(value: unknown): value is Report {
  return !!value && typeof value === 'object' && Array.isArray((value as Report).findings);
}

function runNow(): void {
  if (!ready.value) return;
  const result = runSnippet(source.value, getDry());
  if (!result.ok) {
    error.value = result.error;
    return;
  }

  error.value = '';
  try {
    const value = result.value as {
      ir?: () => Toolpath;
      gcode?: () => string[];
      simulate?: () => Metrics;
      verify?: (...args: unknown[]) => Report;
    };

    const ir = isToolpath(result.value) ? result.value : typeof value?.ir === 'function' ? value.ir() : undefined;
    if (ir && canvas.value) {
      const ctx = canvas.value.getContext('2d');
      if (ctx) drawIr(ctx, ir, canvas.value.width, canvas.value.height);
    }
    irText.value = ir ? JSON.stringify(ir, null, 2) : '';

    if (Array.isArray(result.value) && result.value.every((line) => typeof line === 'string')) {
      gcode.value = result.value;
    } else {
      gcode.value = typeof value?.gcode === 'function' ? value.gcode() : [];
    }

    metricsText.value = isMetrics(result.value)
      ? JSON.stringify(result.value, null, 2)
      : wants('metrics') && typeof value?.simulate === 'function'
        ? JSON.stringify(value.simulate(), null, 2)
        : ir && wants('metrics')
          ? JSON.stringify(getDry().resolveMetricsIr(JSON.stringify(ir)), null, 2)
          : '';

    verifyText.value = isReport(result.value)
      ? JSON.stringify(result.value, null, 2)
      : wants('verify') && typeof value?.verify === 'function'
        ? JSON.stringify(value.verify('generic', 0, 0, [[0, 250], [0, 210], [0, 220]]), null, 2)
        : '';
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  }
}

onMounted(async () => {
  const testing = isTestMode();
  if (editorHost.value && !testing) {
    view.value = new EditorView({
      parent: editorHost.value,
      state: EditorState.create({
        doc: source.value,
        extensions: [
          basicSetup,
          javascript({ typescript: true }),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) {
              source.value = update.state.doc.toString();
              schedule();
            }
          }),
        ],
      }),
    });
  }

  try {
    if (!testing) await initDryEngine();
    ready.value = true;
    runNow();
  } catch (e) {
    error.value = `couldn't load the Dry engine (wasm): ${e instanceof Error ? e.message : String(e)}`;
  }
});

function reset(): void {
  const next = seed();
  source.value = next;
  view.value?.dispatch({ changes: { from: 0, to: view.value.state.doc.length, insert: next } });
  schedule();
}
</script>

<template>
  <ClientOnly>
    <div class="live">
      <div class="live-code">
        <div class="live-bar">
          <span>TypeScript</span>
          <button type="button" @click="reset">Reset</button>
        </div>
        <div ref="editorHost" class="live-editor"></div>
      </div>
      <div class="live-demo">
        <canvas ref="canvas" width="360" height="240"></canvas>
        <div class="live-tabs">
          <button
            v-for="item in tabs"
            :key="item"
            type="button"
            :class="{ on: tab === item }"
            @click="tab = item"
          >
            {{ item }}
          </button>
        </div>
        <pre v-if="tab === 'gcode'" class="live-out">{{ gcode.join('\n') }}</pre>
        <pre v-else-if="tab === 'ir'" class="live-out">{{ irText }}</pre>
        <pre v-else-if="tab === 'metrics'" class="live-out">{{ metricsText }}</pre>
        <pre v-else-if="tab === 'verify'" class="live-out">{{ verifyText }}</pre>
        <div v-if="error" class="live-error">Error: {{ error }}</div>
        <div v-else-if="!ready" class="live-loading">loading engine...</div>
      </div>
    </div>
  </ClientOnly>
</template>

<style scoped>
.live {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 12px;
  border: 1px solid var(--vp-c-divider);
  border-radius: 8px;
  padding: 8px;
  margin: 16px 0;
}

@media (max-width: 720px) {
  .live {
    grid-template-columns: 1fr;
  }
}

.live-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 12px;
  opacity: 0.75;
  padding: 2px 4px;
}

.live-bar button {
  border: 1px solid var(--vp-c-divider);
  border-radius: 4px;
  padding: 2px 8px;
}

.live-editor {
  max-height: 320px;
  overflow: auto;
}

.live-demo canvas {
  width: 100%;
  background: #0b0f17;
  border-radius: 6px;
}

.live-tabs {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin: 6px 0;
}

.live-tabs button {
  border: 1px solid var(--vp-c-divider);
  border-radius: 4px;
  font-size: 12px;
  padding: 2px 8px;
}

.live-tabs button.on {
  background: var(--vp-c-brand-1);
  color: #fff;
}

.live-out {
  max-height: 220px;
  overflow: auto;
  font-size: 12px;
}

.live-error {
  color: #ff6b6b;
  background: #2a1a1a;
  border: 1px solid #ff4444;
  border-radius: 6px;
  padding: 8px;
  font-size: 13px;
}

.live-loading {
  opacity: 0.6;
  font-size: 13px;
  padding: 8px;
}
</style>
