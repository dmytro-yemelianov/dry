import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { SnippetWorkerClient } from './snippet-worker-client';
import type { SnippetWorkerRequest, SnippetWorkerResponse } from './snippet-worker-client';

class FakeWorker {
  static instances: FakeWorker[] = [];
  readonly listeners = new Map<string, Array<(event: any) => void>>();
  request: SnippetWorkerRequest | undefined;
  terminated = false;

  constructor() {
    FakeWorker.instances.push(this);
  }

  addEventListener(type: string, listener: (event: any) => void): void {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }

  postMessage(request: SnippetWorkerRequest): void {
    this.request = request;
  }

  terminate(): void {
    this.terminated = true;
  }

  respond(response: SnippetWorkerResponse): void {
    for (const listener of this.listeners.get('message') ?? []) listener({ data: response });
  }
}

beforeEach(() => {
  FakeWorker.instances = [];
  vi.stubGlobal('Worker', FakeWorker);
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

test('returns a worker result and terminates the completed worker', async () => {
  const client = new SnippetWorkerClient();
  const pending = client.run('42', ['ir']);
  const worker = FakeWorker.instances[0];
  worker.respond({
    id: worker.request!.id,
    result: { ok: true, outputs: { ir: null, gcode: [], metrics: null, verify: null } },
  });

  await expect(pending).resolves.toEqual({
    ok: true,
    outputs: { ir: null, gcode: [], metrics: null, verify: null },
  });
  expect(worker.terminated).toBe(true);
});

test('terminates snippets that exceed the execution deadline', async () => {
  vi.useFakeTimers();
  const client = new SnippetWorkerClient();
  const pending = client.run('while (true) {}', []);
  const worker = FakeWorker.instances[0];

  await vi.advanceTimersByTimeAsync(2_000);

  await expect(pending).resolves.toEqual({
    ok: false,
    error: 'snippet execution exceeded 2000 ms and was stopped',
  });
  expect(worker.terminated).toBe(true);
});
