import { test, expect, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { setWasmBinding, type DryWasm } from '@sdk/engine';
import LiveExample from './LiveExample.vue';

const oneSegmentIr = { version: 1, segments: [
  { start: [0, 0, 0], end: [10, 0, 0], travel: false, kind: 'line', width: 0.4, height: 0.2, centre: null, clockwise: false },
] };

setWasmBinding({
  resolve_ir: () => JSON.stringify(oneSegmentIr),
  resolve_gcode: () => ['G1 X10 Y0 E0.5'],
  resolve_metrics: () => JSON.stringify({ total_time_s: 1 }),
  metrics_ir: () => JSON.stringify({ total_time_s: 1 }),
} as unknown as DryWasm);

HTMLCanvasElement.prototype.getContext = vi.fn(() => null) as never;

const mountLiveExample = (props: { code: string; outputs: string[] }) =>
  mount(LiveExample, {
    props,
    global: { stubs: { ClientOnly: { template: '<div><slot /></div>' } } },
  });

test('renders the seeded code and the g-code output, no error banner', async () => {
  const code = `import { Design } from '@dry/sdk';\nnew Design().geometry(0.6,0.2).extruder(true).point(0,0,0.2).point(10,0,0.2)`;
  const wrapper = mountLiveExample({ code, outputs: ['gcode'] });
  await new Promise((resolve) => setTimeout(resolve, 50));
  expect(wrapper.text()).toContain('G1 X10 Y0');
  expect(wrapper.find('.live-error').exists()).toBe(false);
});

test('a broken edit shows an inline error banner instead of throwing', async () => {
  const wrapper = mountLiveExample({ code: `throw new Error('nope')`, outputs: ['gcode'] });
  await new Promise((resolve) => setTimeout(resolve, 50));
  expect(wrapper.find('.live-error').text()).toMatch(/nope/);
});

test('accepts default slot text as inline source', async () => {
  const wrapper = mount(LiveExample, {
    props: { outputs: ['gcode'] },
    slots: {
      default: `import { Design } from '@dry/sdk';\nnew Design().geometry(0.6,0.2).extruder(true).point(0,0,0.2).point(10,0,0.2)`,
    },
    global: { stubs: { ClientOnly: { template: '<div><slot /></div>' } } },
  });
  await new Promise((resolve) => setTimeout(resolve, 50));
  expect(wrapper.text()).toContain('G1 X10 Y0');
  expect(wrapper.find('.live-error').exists()).toBe(false);
});

test('renders a direct Toolpath result in the IR pane', async () => {
  const wrapper = mountLiveExample({ code: `({ version: 1, segments: [{ start: [0,0,0], end: [2,0,0], travel: false, kind: 'line' }] })`, outputs: ['ir'] });
  await new Promise((resolve) => setTimeout(resolve, 50));
  expect(wrapper.find('.live-out').text()).toContain('"segments"');
  expect(wrapper.find('.live-error').exists()).toBe(false);
});
