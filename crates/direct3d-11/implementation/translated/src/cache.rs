//! The cache of plan section 8 in this front: shaders converted once across runs, and every state
//! combination a program draws recorded and built at its next start (plan 4.4, step 3).

use crate::error::WARNING_PREFIX;
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use wcontainers_gpu::{
    Backend, BindingLayoutKey, BlendDesc, DepthStencilDesc, MAX_BINDING_SETS, PrepareDesc,
    RasterDesc, TargetLayout, Topology, VariantKey, VertexLayoutDesc,
};
use wcontainers_shaders::cache::{self as file, Cache, Kind};
use wcontainers_shaders::{ConvertError, Converted, Source};

use wcontainers_gpu::Object;

use crate::object::{Kept, SharedGraveyard};

/// The process's cache: one file per program, which every device of the process shares.
struct Store {
    cache: Cache,
    /// Where the entries this process adds are appended.
    out: Box<dyn Write + Send>,
    /// A device has built the record: the program's start is past.
    started: bool,
    /// Appending failed once (warned); later entries stay in memory only.
    failed: bool,
}

static STORE: Mutex<Option<Store>> = Mutex::new(None);

fn store() -> std::sync::MutexGuard<'static, Option<Store>> {
    STORE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Uses `cache` as the process's cache, appending each entry it gains to `out`.
pub fn open(cache: Cache, out: Box<dyn Write + Send>) {
    *store() = Some(Store {
        cache,
        out,
        started: false,
        failed: false,
    });
}

fn append(s: &mut Store, bytes: &[u8]) {
    if bytes.is_empty() || s.failed {
        return;
    }
    if let Err(e) = s.out.write_all(bytes).and_then(|()| s.out.flush()) {
        eprintln!("{WARNING_PREFIX}: the cache file cannot be appended to ({e}); kept in memory");
        s.failed = true;
    }
}

/// `bytecode` converted as `key`, from the cache where it holds it.
pub(crate) fn convert(source: Source<'_>, key: &VariantKey) -> Result<Converted, ConvertError> {
    let mut g = store();
    let Some(s) = g.as_mut() else {
        return wcontainers_shaders::convert(source, key, None);
    };
    let (converted, appended) = file::convert_cached(&mut s.cache, source, key, None)?;
    append(s, &appended);
    Ok(converted)
}

/// A module's content key (`ShaderModule::key`), which this front makes with blake3.
pub(crate) type ContentKey = [u8; blake3::OUT_LEN];

/// The content keys of a program's vertex and pixel modules.
pub(crate) type ModuleKeys = (ContentKey, Option<ContentKey>);

/// Tells this front's records from other fronts' in one file.
const FRONT: [u8; 4] = *b"D311";

/// A state combination drawn, in interface terms: the program's two modules by content key and
/// the blocks, targets, topology and binding layouts of its pipeline.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct Record {
    front: [u8; FRONT.len()],
    pub vertex: ContentKey,
    pub fragment: Option<ContentKey>,
    pub blend: BlendDesc,
    pub depth_stencil: DepthStencilDesc,
    pub raster: RasterDesc,
    pub vertex_layout: VertexLayoutDesc,
    pub targets: TargetLayout,
    pub topology: Topology,
    pub unfilterable: [u128; MAX_BINDING_SETS],
}

impl Record {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        program: ModuleKeys,
        blend: BlendDesc,
        depth_stencil: DepthStencilDesc,
        raster: RasterDesc,
        vertex_layout: VertexLayoutDesc,
        targets: TargetLayout,
        topology: Topology,
        layouts: [BindingLayoutKey; MAX_BINDING_SETS],
    ) -> Self {
        Record {
            front: FRONT,
            vertex: program.0,
            fragment: program.1,
            blend,
            depth_stencil,
            raster,
            vertex_layout,
            targets,
            topology,
            unfilterable: layouts.map(|k| k.unfilterable_textures),
        }
    }
}

/// Adds a combination drawn to the record.
pub(crate) fn record(r: &Record) {
    let mut g = store();
    if let Some(s) = g.as_mut() {
        let appended = file::record_combination(&mut s.cache, r);
        append(s, &appended);
    }
}

/// What the program's start built from the record, held until the program makes the same
/// shaders: back-end pipelines are keyed by their objects, so a program built here is the one
/// the first draw of its pair takes.
pub(crate) struct Warm<B: Backend> {
    /// Programs by their modules' content keys.
    pub programs: HashMap<ModuleKeys, Kept<B, B::Program>>,
    /// Each combination built, and whether it was drawn since.
    pub records: HashMap<Record, bool>,
    /// The blocks the pipelines were built with, held so equal blocks made later are the same.
    blocks: Vec<WarmBlocks<B>>,
    /// Records that could not be built (a shader missing from the cache, or refused).
    pub unbuilt: u64,
}

struct WarmBlocks<B: Backend> {
    _blend: Kept<B, B::Blend>,
    _depth_stencil: Kept<B, B::DepthStencil>,
    _raster: Kept<B, B::Raster>,
    _vertex_layout: Kept<B, B::VertexLayout>,
}

impl<B: Backend> Default for Warm<B> {
    fn default() -> Self {
        Warm {
            programs: HashMap::new(),
            records: HashMap::new(),
            blocks: Vec::new(),
            unbuilt: 0,
        }
    }
}

impl<B: Backend> Warm<B> {
    /// Builds every recorded combination at the program's first device; later devices of the
    /// process build nothing. `id` numbers the objects made.
    pub fn start(
        backend: &mut B,
        graveyard: &SharedGraveyard<B>,
        mut id: impl FnMut() -> u64,
    ) -> Self {
        let mut warm = Warm::default();
        let mut g = store();
        let Some(s) = g.as_mut().filter(|s| !s.started) else {
            return warm;
        };
        s.started = true;
        let records: Vec<Record> = file::combinations::<Record>(&s.cache)
            .filter(|r| r.front == FRONT)
            .collect();
        let mut shaders: HashMap<ContentKey, Option<B::Shader>> = HashMap::new();
        for r in records {
            let mut shader = |key: ContentKey| {
                shaders
                    .entry(key)
                    .or_insert_with(|| {
                        let payload = s.cache.get(Kind::Shader, &key)?;
                        let mut c = file::decode(key, payload).ok()?;
                        backend.create_shader(&c.modules.pop()?).ok()
                    })
                    .clone()
            };
            let Some(vertex) = shader(r.vertex) else {
                warm.unbuilt += 1;
                continue;
            };
            let fragment = match r.fragment {
                None => None,
                Some(k) => match shader(k) {
                    Some(f) => Some(f),
                    None => {
                        warm.unbuilt += 1;
                        continue;
                    }
                },
            };
            let pair = (r.vertex, r.fragment);
            if !warm.programs.contains_key(&pair) {
                let Ok(p) = backend.create_program(&wcontainers_gpu::Program::Render {
                    vertex: &vertex,
                    fragment: fragment.as_ref(),
                }) else {
                    warm.unbuilt += 1;
                    continue;
                };
                warm.programs
                    .insert(pair, Kept::new(p, id(), Object::Program, graveyard));
            }
            let blocks = (
                backend.create_blend(&r.blend),
                backend.create_depth_stencil(&r.depth_stencil),
                backend.create_raster(&r.raster),
                backend.create_vertex_layout(&r.vertex_layout),
            );
            let (Ok(blend), Ok(depth_stencil), Ok(raster), Ok(vertex_layout)) = blocks else {
                warm.unbuilt += 1;
                continue;
            };
            let mut layouts = [BindingLayoutKey::default(); MAX_BINDING_SETS];
            for (k, u) in layouts.iter_mut().zip(r.unfilterable) {
                k.unfilterable_textures = u;
            }
            backend.prepare(&PrepareDesc {
                program: &warm.programs[&pair].handle,
                blend: &blend,
                depth_stencil: &depth_stencil,
                raster: &raster,
                vertex_layout: &vertex_layout,
                targets: r.targets,
                topology: r.topology,
                layouts,
            });
            warm.blocks.push(WarmBlocks {
                _blend: Kept::new(blend, id(), Object::Blend, graveyard),
                _depth_stencil: Kept::new(depth_stencil, id(), Object::DepthStencil, graveyard),
                _raster: Kept::new(raster, id(), Object::Raster, graveyard),
                _vertex_layout: Kept::new(vertex_layout, id(), Object::VertexLayout, graveyard),
            });
            warm.records.insert(r, false);
        }
        warm
    }

    /// How many recorded combinations were built at the start, and how many of them were drawn.
    pub fn used(&self) -> (u64, u64) {
        let drawn = self.records.values().filter(|&&d| d).count() as u64;
        (self.records.len() as u64, drawn)
    }
}
