<script setup lang="ts">
import { ref, shallowRef, onMounted, onBeforeUnmount, computed } from 'vue';
import { EditorView, basicSetup } from 'codemirror';
import { javascript } from '@codemirror/lang-javascript';
import { EditorState } from '@codemirror/state';
import { getDry, initDryEngine } from './dry-engine';
import { runSnippet } from './run-snippet';
import { drawIr, presetAngles } from './render-ir';
import type { ViewPreset } from './render-ir';
import type { Metrics, Report, Toolpath } from '@sdk/ops';

const EXAMPLES = import.meta.glob('../../examples/*.ts', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;

const props = withDefaults(defineProps<{ src?: string; code?: string; outputs?: string[] }>(), {
  outputs: () => ['gcode', 'ir', 'metrics'],
});

function seed(): string {
  if (props.code) return props.code.trim();
  const inline = slotSourceHost.value?.textContent?.trim();
  if (inline) return inline;
  const hit = Object.entries(EXAMPLES).find(([key]) => key.endsWith(`/${props.src}.ts`));
  return (hit?.[1] ?? `// example '${props.src}' not found`).trim();
}

const source = ref(props.code?.trim() ?? '');
const error = ref('');
const tab = ref(props.outputs[0]);
const gcode = ref<string[]>([]);
const irText = ref('');
const metricsText = ref('');
const verifyText = ref('');
const canvas = ref<HTMLCanvasElement | null>(null);
const editorHost = ref<HTMLElement | null>(null);
const slotSourceHost = ref<HTMLElement | null>(null);
const ready = ref(false);
const view = shallowRef<EditorView | null>(null);
const lastIr = shallowRef<Toolpath | null>(null);
const viewPresets: ViewPreset[] = ['xy', 'xz', 'yz', 'iso'];
const viewPreset = ref<ViewPreset>('xy');
const zoom = ref(1);
const panX = ref(0);
const panY = ref(0);
const yawDeg = ref(0);
const pitchDeg = ref(0);
const rollDeg = ref(0);
const playHead = ref<number | null>(null);
const playing = ref(false);

let timer: ReturnType<typeof setTimeout> | undefined;
let playTimer: ReturnType<typeof setInterval> | undefined;
let dragStart: { x: number; y: number } | null = null;

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
  const ctx = canvas.value?.getContext('2d');
  if (!ctx || !canvas.value) return;
  if (!lastIr.value) {
    ctx.clearRect(0, 0, canvas.value.width, canvas.value.height);
    return;
  }
  drawIr(ctx, lastIr.value, canvas.value.width, canvas.value.height, {
    view: viewPreset.value,
    zoom: zoom.value,
    panX: panX.value,
    panY: panY.value,
    yawDeg: yawDeg.value,
    pitchDeg: pitchDeg.value,
    rollDeg: rollDeg.value,
    maxSegments: playHead.value ?? undefined,
    activeSegment: playHead.value ? playHead.value - 1 : undefined,
  });
}

function fitView(): void {
  zoom.value = 1;
  panX.value = 0;
  panY.value = 0;
  drawCurrentIr();
}

function setView(next: ViewPreset): void {
  viewPreset.value = next;
  const preset = presetAngles(next);
  yawDeg.value = preset.yawDeg;
  pitchDeg.value = preset.pitchDeg;
  rollDeg.value = preset.rollDeg;
  fitView();
}

function zoomBy(scale: number): void {
  zoom.value = Math.max(0.2, Math.min(8, zoom.value * scale));
  drawCurrentIr();
}

function panBy(dx: number, dy: number): void {
  panX.value += dx;
  panY.value += dy;
  drawCurrentIr();
}

function orbitBy(deltaYaw: number, deltaPitch: number): void {
  yawDeg.value = (yawDeg.value + deltaYaw) % 360;
  pitchDeg.value = Math.max(-89, Math.min(89, pitchDeg.value + deltaPitch));
  drawCurrentIr();
}

function rollBy(deg: number): void {
  rollDeg.value = (rollDeg.value + deg) % 360;
  drawCurrentIr();
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

function onCanvasPointerDown(event: PointerEvent): void {
  dragStart = { x: event.clientX, y: event.clientY };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function onCanvasPointerMove(event: PointerEvent): void {
  if (!dragStart) return;
  const dx = event.clientX - dragStart.x;
  const dy = event.clientY - dragStart.y;
  if (event.shiftKey) panBy(dx, dy);
  else orbitBy(dx * 0.35, -dy * 0.35);
  dragStart = { x: event.clientX, y: event.clientY };
}

function onCanvasPointerUp(event: PointerEvent): void {
  dragStart = null;
  (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
}

function onCanvasWheel(event: WheelEvent): void {
  zoomBy(event.deltaY < 0 ? 1.12 : 0.89);
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
  const seeded = seed();
  source.value = seeded;
  const testing = isTestMode();
  if (editorHost.value && !testing) {
    view.value = new EditorView({
      parent: editorHost.value,
      state: EditorState.create({
        doc: seeded,
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

onBeforeUnmount(stopPlayback);

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
      <div ref="slotSourceHost" class="live-slot-source"><slot /></div>
      <div class="live-code">
        <div class="live-bar">
          <span>TypeScript</span>
          <button type="button" @click="reset">Reset</button>
        </div>
        <div ref="editorHost" class="live-editor"></div>
      </div>
      <div class="live-demo">
        <div class="live-canvas-shell">
          <canvas
            ref="canvas"
            width="900"
            height="560"
            @pointerdown="onCanvasPointerDown"
            @pointermove="onCanvasPointerMove"
            @pointerup="onCanvasPointerUp"
            @pointercancel="onCanvasPointerUp"
            @wheel.prevent="onCanvasWheel"
          ></canvas>
          <div class="live-view-controls" aria-label="Canvas view controls">
            <button
              v-for="item in viewPresets"
              :key="item"
              type="button"
              :class="{ on: viewPreset === item }"
              :title="`View ${item.toUpperCase()}`"
              @click="setView(item)"
            >
              {{ item.toUpperCase() }}
            </button>
            <button type="button" title="Fit" @click="fitView">Fit</button>
            <button type="button" title="Zoom in" @click="zoomBy(1.2)">+</button>
            <button type="button" title="Zoom out" @click="zoomBy(0.84)">-</button>
            <button type="button" title="Pan left" @click="panBy(-28, 0)">L</button>
            <button type="button" title="Pan right" @click="panBy(28, 0)">R</button>
            <button type="button" title="Pan up" @click="panBy(0, -28)">U</button>
            <button type="button" title="Pan down" @click="panBy(0, 28)">D</button>
            <button type="button" title="Orbit left" @click="orbitBy(-15, 0)">Yaw-</button>
            <button type="button" title="Orbit right" @click="orbitBy(15, 0)">Yaw+</button>
            <button type="button" title="Orbit up" @click="orbitBy(0, 12)">Pitch+</button>
            <button type="button" title="Orbit down" @click="orbitBy(0, -12)">Pitch-</button>
            <button type="button" title="Roll left" @click="rollBy(-15)">Roll-</button>
            <button type="button" title="Roll right" @click="rollBy(15)">Roll+</button>
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
    width: min(1180px, calc(100vw - 64px));
    margin-left: 50%;
    transform: translateX(-50%);
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

.live-canvas-shell {
  position: relative;
  min-height: 340px;
  aspect-ratio: 16 / 10;
  background: #0b0f17;
  border-radius: 6px;
  overflow: hidden;
}

.live-demo canvas {
  display: block;
  width: 100%;
  height: 100%;
  touch-action: none;
  cursor: grab;
}

.live-demo canvas:active {
  cursor: grabbing;
}

.live-view-controls {
  position: absolute;
  left: 8px;
  right: 8px;
  bottom: 44px;
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  pointer-events: none;
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
