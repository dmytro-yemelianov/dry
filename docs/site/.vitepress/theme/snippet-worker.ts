/// <reference lib="webworker" />

import { getDry, initDryEngine } from './dry-engine';
import { runSnippet } from './run-snippet';
import { resolveSnippetOutputs } from './snippet-output';
import type { SnippetWorkerRequest, SnippetWorkerResponse } from './snippet-worker-client';

self.addEventListener('message', async (event: MessageEvent<SnippetWorkerRequest>) => {
  const { id, source, outputs } = event.data;
  let response: SnippetWorkerResponse;
  try {
    await initDryEngine();
    const result = runSnippet(source, getDry());
    response = result.ok
      ? { id, result: { ok: true, outputs: resolveSnippetOutputs(result.value, outputs, getDry()) } }
      : { id, result };
  } catch (error) {
    response = { id, result: { ok: false, error: error instanceof Error ? error.message : String(error) } };
  }
  self.postMessage(response);
});
