//! The shader cache of plan section 8 in this front: each shader is converted once per variant, and
//! each DXIL module's views read once, across the process's pipelines and, where the host gives a
//! file, across runs. Pipelines are declared and built when created (plan 8: Direct3D 12 does not
//! have Direct3D 11's first-draw problem), so no state combinations are recorded.

use std::io::Write;
use std::sync::{Mutex, MutexGuard, PoisonError};

use wcontainers_gpu::VariantKey;
use wcontainers_shaders::cache::{self as file, Cache};
use wcontainers_shaders::container::DxilViews;
use wcontainers_shaders::{ConvertError, Converted, Source};

/// The process's cache, shared by every device.
struct Store {
    cache: Cache,
    /// Where the entries this process adds are appended; none keeps them in memory only.
    out: Option<Box<dyn Write + Send>>,
    /// Conversions made and taken from the cache.
    converted: u64,
    decoded: u64,
}

static STORE: Mutex<Option<Store>> = Mutex::new(None);

fn store() -> MutexGuard<'static, Option<Store>> {
    STORE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Uses `cache` as the process's cache, appending each entry it gains to `out`. Called before the
/// first conversion; without it the process keeps its conversions in memory only.
pub fn open(cache: Cache, out: Box<dyn Write + Send>) {
    *store() = Some(Store {
        cache,
        out: Some(out),
        converted: 0,
        decoded: 0,
    });
}

/// The process's store, made in memory where no file was opened.
fn opened(g: &mut Option<Store>) -> &mut Store {
    g.get_or_insert_with(|| Store {
        cache: Cache::default(),
        out: None,
        converted: 0,
        decoded: 0,
    })
}

impl Store {
    /// Appends `appended` to the file, keeping the cache in memory only once it cannot be.
    fn append(&mut self, appended: &[u8]) {
        if let Some(out) = &mut self.out
            && let Err(e) = out.write_all(appended).and_then(|()| out.flush())
        {
            eprintln!(
                "wcontainers direct3d 12: the cache file cannot be appended to ({e}); kept in memory"
            );
            self.out = None;
        }
    }
}

/// `source` converted as `variant`, from the cache where it holds it.
pub(crate) fn convert(source: Source<'_>, variant: &VariantKey) -> Result<Converted, ConvertError> {
    let mut g = store();
    let s = opened(&mut g);
    let (converted, appended) = file::convert_cached(&mut s.cache, source, variant, None)?;
    if appended.is_empty() {
        s.decoded += 1;
        return Ok(converted);
    }
    s.converted += 1;
    s.append(&appended);
    Ok(converted)
}

/// The views `code`'s DXIL module declares, from the cache where it holds them: each module is
/// read once, however many pipelines and stages use it.
pub(crate) fn dxil_views(code: &[u8]) -> Option<DxilViews> {
    let mut g = store();
    let s = opened(&mut g);
    let (views, appended) = file::dxil_views_cached(&mut s.cache, code);
    if !appended.is_empty() {
        s.append(&appended);
    }
    views
}

/// How many shaders the process converted and how many it took from the cache.
pub fn counts() -> (u64, u64) {
    store()
        .as_ref()
        .map_or((0, 0), |s| (s.converted, s.decoded))
}
