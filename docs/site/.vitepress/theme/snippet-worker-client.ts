import type { SnippetOutputs } from './snippet-output';

export interface SnippetWorkerRequest {
  id: number;
  source: string;
  outputs: string[];
}

export interface SnippetWorkerResponse {
  id: number;
  result: { ok: true; outputs: SnippetOutputs } | { ok: false; error: string };
}

export type SnippetExecutionResult = SnippetWorkerResponse['result'];

const EXECUTION_TIMEOUT_MS = 2_000;

/** Execute editable snippets off the UI thread and terminate non-responsive workers. */
export class SnippetWorkerClient {
  private worker: Worker | null = null;
  private nextId = 0;
  private pending: { id: number; resolve: (result: SnippetExecutionResult) => void; timer: ReturnType<typeof setTimeout> } | null = null;

  run(source: string, outputs: string[]): Promise<SnippetExecutionResult> {
    this.reset({ ok: false, error: 'snippet execution superseded by a newer edit' });
    const worker = new Worker(new URL('./snippet-worker.ts', import.meta.url), { type: 'module' });
    const id = ++this.nextId;
    this.worker = worker;

    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        this.reset();
        resolve({ ok: false, error: `snippet execution exceeded ${EXECUTION_TIMEOUT_MS} ms and was stopped` });
      }, EXECUTION_TIMEOUT_MS);
      this.pending = { id, resolve, timer };

      worker.addEventListener('message', (event: MessageEvent<SnippetWorkerResponse>) => {
        if (event.data.id !== this.pending?.id) return;
        const result = event.data.result;
        this.reset();
        resolve(result);
      });
      worker.addEventListener('error', (event) => {
        if (id !== this.pending?.id) return;
        this.reset();
        resolve({ ok: false, error: event.message || 'snippet worker failed' });
      });
      worker.postMessage({ id, source, outputs } satisfies SnippetWorkerRequest);
    });
  }

  dispose(): void {
    this.reset({ ok: false, error: 'snippet execution cancelled' });
  }

  private reset(result?: SnippetExecutionResult): void {
    this.worker?.terminate();
    this.worker = null;
    if (!this.pending) return;
    clearTimeout(this.pending.timer);
    const { resolve } = this.pending;
    this.pending = null;
    if (result) resolve(result);
  }
}
