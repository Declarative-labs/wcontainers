//! Program stages on the Gallium trait: each `create_*_state` hands over the stage st/mesa received,
//! typed by its stage (the Mesa compiler's `Received*` kinds), which the driver's steps at creation
//! prepare once ([`wcontainers_nir::mesa::Received::prepare`]); every draw's variant starts from it.

use std::sync::Arc;

use wcontainers_mesa_compiler::MesaCompiler;
use wcontainers_mesa_compiler_direct::Bridge;
use wcontainers_nir::mesa::{Prepared, Received, ReceivedShader};
use wcontainers_shaders::stage::StreamOutput;

use crate::guest::refuse;
use crate::state::Stage;
use crate::stream;

/// A program stage as the driver keeps it: prepared, with what it streams.
pub struct Shader {
    pub stage: Stage,
    pub prepared: Prepared<Bridge>,
    /// What a vertex, evaluation or geometry program streams while stream output is bound: the
    /// stage helper's declaration, none, or why it cannot.
    pub streamed: Result<Option<StreamOutput>, &'static str>,
}

/// The stage st/mesa handed over, prepared; refused by name where the compiler gives no program.
pub fn create(shader: ReceivedShader<Bridge>) -> Arc<Shader> {
    let prepared = Received::new(Bridge::new(), shader)
        .prepare()
        .unwrap_or_else(|e| refuse(format_args!("a program the driver's steps refused: {e}")));
    let stage = match prepared.stage() {
        wcontainers_nir::mesa::Stage::Vertex => Stage::Vertex,
        wcontainers_nir::mesa::Stage::Fragment => Stage::Fragment,
        wcontainers_nir::mesa::Stage::Compute => Stage::Compute,
        wcontainers_nir::mesa::Stage::Geometry => Stage::Geometry,
        wcontainers_nir::mesa::Stage::TessControl => Stage::TessControl,
        wcontainers_nir::mesa::Stage::TessEvaluation => Stage::TessEvaluation,
    };
    // The last stage before the rasterizer streams its outputs.
    let streams = matches!(
        stage,
        Stage::Vertex | Stage::Geometry | Stage::TessEvaluation
    );
    let streamed = match &prepared.streamed {
        _ if !streams => Ok(None),
        Some(outputs) if outputs.is_empty() => Ok(None),
        Some(outputs) => Ok(Some(stream::declaration(outputs, prepared.strides))),
        None => Err("a streamed value that is neither at a location nor the position"),
    };
    Arc::new(Shader {
        stage,
        prepared,
        streamed,
    })
}
