//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.

/// The header's `D3DMATRIX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3d10/d3d10-d3dmatrix).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D3Dmatrix {
    pub anonymous: crate::structures::D3DmatrixAnonymous,
}

/// A C union of `D3DMATRIX`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D3DmatrixAnonymous {
    pub anonymous: crate::structures::D3DmatrixAnonymousAnonymous,
    pub m: [f32; crate::constants::D3DMATRIX_ANONYMOUS_E_UNION_M_LENGTH],
}

/// A nested struct of `_Anonymous_e__Union`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D3DmatrixAnonymousAnonymous {
    pub _11: f32,
    pub _12: f32,
    pub _13: f32,
    pub _14: f32,
    pub _21: f32,
    pub _22: f32,
    pub _23: f32,
    pub _24: f32,
    pub _31: f32,
    pub _32: f32,
    pub _33: f32,
    pub _34: f32,
    pub _41: f32,
    pub _42: f32,
    pub _43: f32,
    pub _44: f32,
}

/// The header's `D3DVECTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3d10/d3d10-d3dvector).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D3Dvector {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// The header's `D3D_SHADER_MACRO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ns-d3dcommon-d3d_shader_macro).
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderMacro {
    pub name: std::ffi::CString,
    pub definition: std::ffi::CString,
}
