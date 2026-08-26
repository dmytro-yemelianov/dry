//! Low-latency chunked streaming G-code emitter (D3.1, `docs/04-tasks.md` — unplanned series D2–D4).
//!
//! Emits G-code in bounded line-chunks, suitable for direct WebSocket/HTTP streaming to
//! Klipper Moonraker, OctoPrint, or CNC serial controllers with constant $O(1)$ memory usage.

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
