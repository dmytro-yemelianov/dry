import { flushPromises, mount } from '@vue/test-utils';
import { EditorView } from 'codemirror';
import { nextTick } from 'vue';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  initDryEngine: vi.fn(() => Promise.resolve()),
  runSnippet: vi.fn(() => ({ ok: true, value: 42 })),
  viewerDispose: vi.fn(),
}));

vi.mock('./dry-engine', () => ({
  getDry: () => ({}),
  initDryEngine: mocks.initDryEngine,
}));

vi.mock('./three-ir-viewer', () => ({
  ThreeIrViewer: class {
    clear() {}
    dispose() { mocks.viewerDispose(); }
    fit() {}
    render() {}
    setView() {}
  },
}));

vi.mock('./run-snippet', () => ({ runSnippet: mocks.runSnippet }));

beforeEach(() => {
  mocks.initDryEngine.mockReset().mockResolvedValue(undefined);
  mocks.runSnippet.mockReset().mockReturnValue({ ok: true, value: 42 });
  mocks.viewerDispose.mockReset();
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
});

test('does not run a late engine initialization after unmount', async () => {
  vi.stubEnv('MODE', 'development');
  let finish!: () => void;
  mocks.initDryEngine.mockReturnValueOnce(new Promise<void>((resolve) => { finish = resolve; }));
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

  expect(mocks.runSnippet).not.toHaveBeenCalled();
});
