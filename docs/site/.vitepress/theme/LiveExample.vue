<script setup lang="ts">
import { ref, shallowRef, onMounted, onBeforeUnmount, computed, nextTick } from 'vue';
import { EditorView, basicSetup } from 'codemirror';
import { javascript } from '@codemirror/lang-javascript';
import { Compartment, EditorState } from '@codemirror/state';
import { python } from '@codemirror/lang-python';
import { getDry, initDryEngine } from './dry-engine';
import { runSnippet } from './run-snippet';
import { ThreeIrViewer } from './three-ir-viewer';
import type { CadViewPreset } from './three-ir-viewer';
import type { Metrics, Report, Toolpath } from '@sdk/ops';

const TS_EXAMPLES = import.meta.glob('../../examples/*.ts', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;
const PY_EXAMPLES = import.meta.glob('../../examples/*.py', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;
type CodeLanguage = 'ts' | 'py';
const LANGUAGE_LABELS: Record<CodeLanguage, string> = { ts: 'TypeScript', py: 'Python' };

const props = withDefaults(defineProps<{ src?: string; code?: string; outputs?: string[] }>(), {
  outputs: () => ['gcode', 'ir', 'metrics'],
});

function seed(): string {
  if (props.code) return props.code.trim();
  const inline = slotSourceHost.value?.textContent?.trim();
  if (inline) return inline;
  const hit = Object.entries(TS_EXAMPLES).find(([key]) => key.endsWith(`/${props.src}.ts`));
  return (hit?.[1] ?? `// example '${props.src}' not found`).trim();
}

function sourceFor(lang: CodeLanguage): string {
  if (lang === 'ts') return source.value || seed();
  const hit = Object.entries(PY_EXAMPLES).find(([key]) => key.endsWith(`/${props.src}.py`));
  return (hit?.[1] ?? '# Python example not available for this snippet.').trim();
}

const source = ref(props.code?.trim() ?? '');
const selectedLanguage = ref<CodeLanguage>('ts');
const availableLanguages = computed<CodeLanguage[]>(() => props.code ? ['ts'] : ['ts', 'py']);
const codeCollapsed = ref(false);
const error = ref('');
const tab = ref(props.outputs[0]);
const gcode = ref<string[]>([]);
const irText = ref('');
const metricsText = ref('');
const verifyText = ref('');
const viewportHost = ref<HTMLElement | null>(null);
const editorHost = ref<HTMLElement | null>(null);
const slotSourceHost = ref<HTMLElement | null>(null);
const ready = ref(false);
const view = shallowRef<EditorView | null>(null);
const languageCompartment = new Compartment();
const editableCompartment = new Compartment();
const viewer = shallowRef<ThreeIrViewer | null>(null);
const lastIr = shallowRef<Toolpath | null>(null);
const viewPresets: Array<{ key: CadViewPreset; label: string }> = [
  { key: 'top', label: 'Top' },
  { key: 'front', label: 'Front' },
  { key: 'right', label: 'Right' },
  { key: 'iso', label: 'Iso' },
];
const viewPreset = ref<CadViewPreset>('iso');
const playHead = ref<number | null>(null);
const playing = ref(false);

let timer: ReturnType<typeof setTimeout> | undefined;
let playTimer: ReturnType<typeof setInterval> | undefined;

const tabs = computed(() => props.outputs);
const segmentCount = computed(() => lastIr.value?.segments.length ?? 0);
const visibleSegments = computed(() => playHead.value ?? segmentCount.value);
const wants = (name: string) => props.outputs.includes(name);
const isTestMode = (): boolean => {
  const meta = import.meta as ImportMeta & { env?: { MODE?: string } };
  return meta.env?.MODE === 'test';
};

function schedule(): void {
  clearTimeout(timer);
  timer = setTimeout(runNow, 250);
}

function languageExtension(lang: CodeLanguage) {
  return lang === 'py' ? python() : javascript({ typescript: true });
}

function setCodeLanguage(lang: CodeLanguage): void {
  selectedLanguage.value = lang;
  const doc = sourceFor(lang);
  view.value?.dispatch({
    changes: { from: 0, to: view.value.state.doc.length, insert: doc },
    effects: [
      languageCompartment.reconfigure(languageExtension(lang)),
      editableCompartment.reconfigure(EditorState.readOnly.of(lang !== 'ts')),
    ],
  });
}

function toggleCodeCollapsed(): void {
  codeCollapsed.value = !codeCollapsed.value;
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

function drawCurrentIr(): void {
  if (!lastIr.value || !viewer.value) return;
  viewer.value.render(lastIr.value, {
    maxSegments: playHead.value ?? undefined,
    activeSegment: playHead.value ? playHead.value - 1 : undefined,
  });
}

function fitView(): void {
  viewer.value?.fit();
}

function setView(next: CadViewPreset): void {
  viewPreset.value = next;
  viewer.value?.setView(next);
}

function setPlayHead(next: number | null): void {
  const max = segmentCount.value;
  playHead.value = next === null ? null : Math.max(0, Math.min(max, next));
  drawCurrentIr();
}

function stepBy(delta: number): void {
  const current = playHead.value ?? segmentCount.value;
  setPlayHead(current + delta);
}

function jumpStart(): void {
  stopPlayback();
  setPlayHead(segmentCount.value > 0 ? 1 : 0);
}

function jumpEnd(): void {
  stopPlayback();
  setPlayHead(null);
}

function stopPlayback(): void {
  playing.value = false;
  if (playTimer) {
    clearInterval(playTimer);
    playTimer = undefined;
  }
}

function togglePlayback(): void {
  if (playing.value) {
    stopPlayback();
    return;
  }
  if (!segmentCount.value) return;
  if (playHead.value === null || playHead.value >= segmentCount.value) setPlayHead(1);
  playing.value = true;
  playTimer = setInterval(() => {
    if (!playing.value) return;
    const next = (playHead.value ?? 0) + 1;
    if (next >= segmentCount.value) {
      setPlayHead(null);
      stopPlayback();
      return;
    }
    setPlayHead(next);
  }, 220);
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
    stopPlayback();
    playHead.value = null;
    lastIr.value = ir ?? null;
    drawCurrentIr();
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
  await nextTick();
  const seeded = seed();
  source.value = seeded;
  const testing = isTestMode();
  if (viewportHost.value && !testing) viewer.value = new ThreeIrViewer(viewportHost.value);
  if (editorHost.value && !testing) {
    view.value = new EditorView({
      parent: editorHost.value,
      state: EditorState.create({
        doc: seeded,
        extensions: [
          basicSetup,
          languageCompartment.of(languageExtension('ts')),
          editableCompartment.of(EditorState.readOnly.of(false)),
          EditorView.updateListener.of((update) => {
            if (update.docChanged && selectedLanguage.value === 'ts') {
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

onBeforeUnmount(() => {
  stopPlayback();
  viewer.value?.dispose();
  viewer.value = null;
});

function reset(): void {
  const next = seed();
  source.value = next;
  selectedLanguage.value = 'ts';
  view.value?.dispatch({ changes: { from: 0, to: view.value.state.doc.length, insert: next } });
  view.value?.dispatch({
    effects: [
      languageCompartment.reconfigure(languageExtension('ts')),
      editableCompartment.reconfigure(EditorState.readOnly.of(false)),
    ],
  });
  schedule();
}
</script>

<template>
  <ClientOnly>
    <div class="live">
      <div ref="slotSourceHost" class="live-slot-source"><slot /></div>
      <div class="live-code" :class="{ collapsed: codeCollapsed }">
        <div class="live-bar">
          <div class="live-language-tabs">
            <button
              v-for="lang in availableLanguages"
              :key="lang"
              type="button"
              :class="{ on: selectedLanguage === lang }"
              @click="setCodeLanguage(lang)"
            >
              {{ LANGUAGE_LABELS[lang] }}
            </button>
          </div>
          <div class="live-code-actions">
            <button type="button" @click="toggleCodeCollapsed">{{ codeCollapsed ? 'Show code' : 'Hide code' }}</button>
            <button type="button" @click="reset">Reset TS</button>
          </div>
        </div>
        <div v-show="!codeCollapsed" ref="editorHost" class="live-editor"></div>
        <div v-if="!codeCollapsed && selectedLanguage !== 'ts'" class="live-code-note">
          Reference only. The live preview runs the TypeScript version.
        </div>
      </div>
      <div class="live-demo">
        <div class="live-canvas-shell">
          <div ref="viewportHost" class="live-viewport"></div>
          <div class="live-view-controls" aria-label="CAD view controls">
            <button
              v-for="item in viewPresets"
              :key="item.key"
              type="button"
              :class="{ on: viewPreset === item.key }"
              :title="`${item.label} view`"
              @click="setView(item.key)"
            >
              {{ item.label }}
            </button>
            <button type="button" title="Fit" @click="fitView">Fit</button>
          </div>
          <div class="live-play-controls" aria-label="Toolpath playback controls">
            <button type="button" title="First segment" @click="jumpStart">|&lt;</button>
            <button type="button" title="Previous segment" @click="stepBy(-1)">&lt;</button>
            <button type="button" :title="playing ? 'Pause' : 'Play'" @click="togglePlayback">{{ playing ? 'Pause' : 'Play' }}</button>
            <button type="button" title="Next segment" @click="stepBy(1)">&gt;</button>
            <button type="button" title="Full path" @click="jumpEnd">&gt;|</button>
            <span>{{ visibleSegments }} / {{ segmentCount }}</span>
          </div>
        </div>
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
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: minmax(280px, 0.78fr) minmax(420px, 1.22fr);
  gap: 14px;
  border: 1px solid var(--vp-c-divider);
  border-radius: 8px;
  padding: 10px;
  margin: 16px 0;
}

@media (min-width: 960px) {
  .live {
    width: min(1180px, calc(100vw - var(--vp-sidebar-width, 272px) - 160px));
    margin-left: 0;
    transform: none;
  }
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
  gap: 8px;
  font-size: 12px;
  opacity: 0.75;
  padding: 2px 4px;
}

.live-language-tabs,
.live-code-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.live-bar button {
  border: 1px solid var(--vp-c-divider);
  border-radius: 4px;
  padding: 2px 8px;
}

.live-language-tabs button.on {
  background: var(--vp-c-brand-1);
  color: #fff;
}

.live-editor {
  max-height: 320px;
  overflow: auto;
}

.live-code.collapsed {
  align-self: start;
}

.live-code-note {
  border-top: 1px solid var(--vp-c-divider);
  font-size: 12px;
  opacity: 0.72;
  padding: 6px 4px 0;
}

.live-canvas-shell {
  position: relative;
  min-height: 340px;
  aspect-ratio: 16 / 10;
  background: #0b0f17;
  border-radius: 6px;
  overflow: hidden;
}

.live-viewport {
  width: 100%;
  height: 100%;
}

.live-viewport :deep(canvas) {
  display: block;
  width: 100% !important;
  height: 100% !important;
  cursor: grab;
}

.live-viewport :deep(canvas:active) {
  cursor: grabbing;
}

.live-view-controls {
  position: absolute;
  left: 8px;
  right: auto;
  bottom: 44px;
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-start;
  gap: 4px;
  pointer-events: none;
  z-index: 2;
}

.live-view-controls button,
.live-play-controls button {
  pointer-events: auto;
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 4px;
  background: rgba(8, 12, 20, 0.78);
  color: #e7edf7;
  font-size: 11px;
  line-height: 1;
  min-width: 28px;
  padding: 6px 7px;
}

.live-view-controls button.on {
  background: #2f8ee8;
  border-color: #67b4ff;
  color: #fff;
}

.live-play-controls {
  position: absolute;
  left: 8px;
  right: 8px;
  bottom: 8px;
  display: flex;
  align-items: center;
  gap: 4px;
  pointer-events: none;
  z-index: 2;
}

.live-play-controls span {
  margin-left: auto;
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 4px;
  background: rgba(8, 12, 20, 0.72);
  color: #d7e2ef;
  font-size: 11px;
  line-height: 1;
  padding: 6px 7px;
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

.live-slot-source {
  display: none;
}
</style>
