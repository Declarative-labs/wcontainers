//! Methods of the types [`crate::resource`] declares, and the reason back ends give for a handle
//! that names no live object ([`GpuError::GONE`]).

use crate::error::GpuError;
use crate::resource::*;

impl GpuError {
    /// Why a back end refuses a call naming a destroyed object, or one it never made.
    pub const GONE: &'static str =
        "a handle names an object destroyed before, or one this back end never made";
}

impl TextureDesc {
    /// The dimension a view of the whole texture has: `Cube` for a cube texture of six layers,
    /// `CubeArray` for more, `D2Array` for a 2D texture of several layers, otherwise `D2` or `D3`.
    pub fn own_view(&self) -> ViewDimension {
        match (self.dimension, self.cube, self.depth_or_layers) {
            (TextureDimension::D3, _, _) => ViewDimension::D3,
            (TextureDimension::D2, true, 6) => ViewDimension::Cube,
            (TextureDimension::D2, true, _) => ViewDimension::CubeArray,
            (TextureDimension::D2, false, 1) => ViewDimension::D2,
            (TextureDimension::D2, false, _) => ViewDimension::D2Array,
        }
    }
}
