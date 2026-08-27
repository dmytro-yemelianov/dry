//! Low-latency chunked streaming G-code emitter (D3.1, `docs/04-tasks.md` — unplanned series D2–D4).
//!
//! Splits an emitted program into bounded line-chunks, sized for WebSocket/HTTP delivery to
//! Klipper Moonraker, OctoPrint, or CNC serial controllers.
//!
//! Chunking here is about *message size*, not memory. The header used to promise "constant $O(1)$
//! memory usage", which this function does not provide and cannot: it emits the whole program
//! through [`emit_stream`] — which itself buffers every line — and returns a `Vec<String>` of all
//! chunks, so the caller holds the entire program at once. Measured peak allocation grows with the
//! segment count (roughly 13x for 10x the segments, 27x for 30x), not flat.
//!
//! For genuinely bounded memory use [`super::emit_stream_to_writer`], which writes each line out
//! as it is produced and never accumulates the program.

use super::{emit_stream, EmitParams};
use crate::codec::CodecError;
use crate::ir::Toolpath;

/// Emit a toolpath as a sequence of fixed-size G-code line blocks.
pub fn emit_gcode_chunks(
    toolpath: &Toolpath,
    params: &EmitParams,
    lines_per_chunk: usize,
) -> Result<Vec<String>, CodecError> {
    let chunk_size = lines_per_chunk.max(1);
    let full_lines = emit_stream(toolpath.segments.iter().cloned().map(Ok), params)?;

    let mut chunks = Vec::new();
    for window in full_lines.chunks(chunk_size) {
        chunks.push(window.join("\n"));
    }

    Ok(chunks)
}
