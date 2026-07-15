import { flushPromises, mount } from '@vue/test-utils';
import { EditorView } from 'codemirror';
import { nextTick } from 'vue';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  runSnippet: vi.fn(() => ({ ok: true, value: 42 })),
  workerRun: vi.fn(() => Promise.resolve({
    ok: true,
    outputs: { ir: null as unknown, gcode: [], metrics: null, verify: null },
  })),
  workerDispose: vi.fn(),
  viewerDispose: vi.fn(),
  viewerRender: vi.fn(),
}));

vi.mock('./dry-engine', () => ({
  getDry: () => ({}),
}));

vi.mock('./snippet-worker-client', () => ({
  SnippetWorkerClient: class {
    run = mocks.workerRun;
    dispose = mocks.workerDispose;
  },
}));

vi.mock('./three-ir-viewer', () => ({
  ThreeIrViewer: class {
    clear() {}
    dispose() { mocks.viewerDispose(); }
    fit() {}
    render(...args: unknown[]) { mocks.viewerRender(...args); }
    setView() {}
  },
}));

vi.mock('./run-snippet', () => ({ runSnippet: mocks.runSnippet }));

beforeEach(() => {
  mocks.runSnippet.mockReset().mockReturnValue({ ok: true, value: 42 });
  mocks.workerRun.mockReset().mockResolvedValue({
    ok: true,
    outputs: { ir: null as unknown, gcode: [], metrics: null, verify: null },
  });
  mocks.workerDispose.mockReset();
  mocks.viewerDispose.mockReset();
  mocks.viewerRender.mockReset();
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
});

test('destroys editor and viewer resources when unmounted', async () => {
  vi.stubEnv('MODE', 'development');
  const destroy = vi.spyOn(EditorView.prototype, 'destroy');
  const { default: LiveExample } = await import('./LiveExample.vue');
  const wrapper = mount(LiveExample, {
    props: { code: '42', outputs: ['gcode'] },
    global: { stubs: { ClientOnly: { template: '<div><slot /></div>' } } },
  });

  await nextTick();
  await nextTick();
  wrapper.unmount();

  expect(destroy).toHaveBeenCalledOnce();
  expect(mocks.viewerDispose).toHaveBeenCalledOnce();
  expect(mocks.workerDispose).toHaveBeenCalledOnce();
});

test('does not render a late worker result after unmount', async () => {
  vi.stubEnv('MODE', 'development');
  let finish!: () => void;
  mocks.workerRun.mockReturnValueOnce(new Promise((resolve) => {
    finish = () => resolve({
      ok: true,
      outputs: {
        ir: { version: 1, segments: [] },
        gcode: [],
        metrics: null,
        verify: null,
      },
    });
  }));
  const { default: LiveExample } = await import('./LiveExample.vue');
  const wrapper = mount(LiveExample, {
    props: { code: '42', outputs: ['gcode'] },
    global: { stubs: { ClientOnly: { template: '<div><slot /></div>' } } },
  });

  await nextTick();
  await nextTick();
  wrapper.unmount();
  finish();
  await flushPromises();

  expect(mocks.viewerRender).not.toHaveBeenCalled();
  expect(mocks.workerDispose).toHaveBeenCalledOnce();
});
