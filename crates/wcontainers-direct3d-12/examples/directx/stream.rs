//! `ID3D12Device2::CreatePipelineState`: a pipeline state stream read into the graphics or compute
//! description `CreateGraphicsPipelineState` and `CreateComputePipelineState` take.

use std::ffi::c_void;
use std::mem::{ManuallyDrop, align_of, size_of};

use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC;
use windows::core::{BOOL, Interface, Result};

/// What a stream describes.
pub enum Pipeline {
    /// The description, and whether the stream gave a primitive topology type.
    Graphics(Box<D3D12_GRAPHICS_PIPELINE_STATE_DESC>, bool),
    Compute(D3D12_COMPUTE_PIPELINE_STATE_DESC),
}

/// Subobjects start at multiples of a pointer's size.
const SUBOBJECT_ALIGN: usize = size_of::<*const c_void>();

/// The values a stream's absent subobjects take, as Direct3D 12 documents them.
fn defaults() -> D3D12_GRAPHICS_PIPELINE_STATE_DESC {
    let target = D3D12_RENDER_TARGET_BLEND_DESC {
        BlendEnable: BOOL(0),
        LogicOpEnable: BOOL(0),
        SrcBlend: D3D12_BLEND_ONE,
        DestBlend: D3D12_BLEND_ZERO,
        BlendOp: D3D12_BLEND_OP_ADD,
        SrcBlendAlpha: D3D12_BLEND_ONE,
        DestBlendAlpha: D3D12_BLEND_ZERO,
        BlendOpAlpha: D3D12_BLEND_OP_ADD,
        LogicOp: D3D12_LOGIC_OP_NOOP,
        RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
    };
    let face = D3D12_DEPTH_STENCILOP_DESC {
        StencilFailOp: D3D12_STENCIL_OP_KEEP,
        StencilDepthFailOp: D3D12_STENCIL_OP_KEEP,
        StencilPassOp: D3D12_STENCIL_OP_KEEP,
        StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS,
    };
    D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        BlendState: D3D12_BLEND_DESC {
            AlphaToCoverageEnable: BOOL(0),
            IndependentBlendEnable: BOOL(0),
            RenderTarget: [target; 8],
        },
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC {
            FillMode: D3D12_FILL_MODE_SOLID,
            CullMode: D3D12_CULL_MODE_BACK,
            DepthClipEnable: BOOL(1),
            ..Default::default()
        },
        DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
            DepthEnable: BOOL(1),
            DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL,
            DepthFunc: D3D12_COMPARISON_FUNC_LESS,
            StencilEnable: BOOL(0),
            StencilReadMask: 0xff,
            StencilWriteMask: 0xff,
            FrontFace: face,
            BackFace: face,
        },
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        ..Default::default()
    }
}

/// Reads a stream: each subobject at most once, of a known type and wholly inside the stream; a
/// compute shader with no vertex shader makes a compute pipeline (other subobjects are ignored),
/// a vertex shader a graphics one. Anything else is `E_INVALIDARG`, as Direct3D 12 answers.
///
/// # Safety
/// The stream points to `SizeInBytes` readable bytes, and its pointers are valid, as the API
/// requires.
pub unsafe fn read(stream: &D3D12_PIPELINE_STATE_STREAM_DESC) -> Result<Pipeline> {
    let bytes = stream
        .pPipelineStateSubobjectStream
        .cast::<u8>()
        .cast_const();
    let len = stream.SizeInBytes;
    if bytes.is_null() || len == 0 {
        return Err(E_INVALIDARG.into());
    }
    let mut g = defaults();
    let mut topology_given = false;
    let mut cs = D3D12_SHADER_BYTECODE::default();
    let mut root: *mut c_void = std::ptr::null_mut();
    let mut seen = 0u64;
    let mut at = 0;
    while at < len {
        if at + 4 > len {
            return Err(E_INVALIDARG.into());
        }
        // SAFETY: four bytes inside the stream.
        let kind = unsafe { bytes.add(at).cast::<u32>().read_unaligned() };
        if kind >= 64 || seen & (1 << kind) != 0 {
            return Err(E_INVALIDARG.into());
        }
        seen |= 1 << kind;
        // The subobject's value, which follows its type at the value's own alignment.
        macro_rules! value {
            ($t:ty) => {{
                let start = (at + 4).next_multiple_of(align_of::<$t>());
                let end = start + size_of::<$t>();
                if end > len {
                    return Err(E_INVALIDARG.into());
                }
                at = end.next_multiple_of(SUBOBJECT_ALIGN);
                // SAFETY: inside the stream, checked above.
                unsafe { bytes.add(start).cast::<$t>().read_unaligned() }
            }};
        }
        match D3D12_PIPELINE_STATE_SUBOBJECT_TYPE(kind as i32) {
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_ROOT_SIGNATURE => root = value!(*mut c_void),
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_VS => g.VS = value!(D3D12_SHADER_BYTECODE),
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_PS => g.PS = value!(D3D12_SHADER_BYTECODE),
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DS => g.DS = value!(D3D12_SHADER_BYTECODE),
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_HS => g.HS = value!(D3D12_SHADER_BYTECODE),
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_GS => g.GS = value!(D3D12_SHADER_BYTECODE),
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_CS => cs = value!(D3D12_SHADER_BYTECODE),
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_STREAM_OUTPUT => {
                g.StreamOutput = value!(D3D12_STREAM_OUTPUT_DESC)
            }
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_BLEND => g.BlendState = value!(D3D12_BLEND_DESC),
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_SAMPLE_MASK => g.SampleMask = value!(u32),
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RASTERIZER => {
                g.RasterizerState = value!(D3D12_RASTERIZER_DESC)
            }
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL => {
                g.DepthStencilState = value!(D3D12_DEPTH_STENCIL_DESC)
            }
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_INPUT_LAYOUT => {
                g.InputLayout = value!(D3D12_INPUT_LAYOUT_DESC)
            }
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_IB_STRIP_CUT_VALUE => {
                g.IBStripCutValue = value!(D3D12_INDEX_BUFFER_STRIP_CUT_VALUE)
            }
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_PRIMITIVE_TOPOLOGY => {
                g.PrimitiveTopologyType = value!(D3D12_PRIMITIVE_TOPOLOGY_TYPE);
                topology_given = true;
            }
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RENDER_TARGET_FORMATS => {
                let f = value!(D3D12_RT_FORMAT_ARRAY);
                g.RTVFormats = f.RTFormats;
                g.NumRenderTargets = f.NumRenderTargets;
            }
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL_FORMAT => {
                g.DSVFormat = value!(windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT)
            }
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_SAMPLE_DESC => {
                g.SampleDesc = value!(DXGI_SAMPLE_DESC)
            }
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_NODE_MASK => g.NodeMask = value!(u32),
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_CACHED_PSO => {
                g.CachedPSO = value!(D3D12_CACHED_PIPELINE_STATE)
            }
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_FLAGS => {
                g.Flags = value!(D3D12_PIPELINE_STATE_FLAGS)
            }
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL1 => {
                let d = value!(D3D12_DEPTH_STENCIL_DESC1);
                // `D3D12_OPTIONS2` reports no depth bounds test.
                if d.DepthBoundsTestEnable.as_bool() {
                    return Err(E_INVALIDARG.into());
                }
                g.DepthStencilState = D3D12_DEPTH_STENCIL_DESC {
                    DepthEnable: d.DepthEnable,
                    DepthWriteMask: d.DepthWriteMask,
                    DepthFunc: d.DepthFunc,
                    StencilEnable: d.StencilEnable,
                    StencilReadMask: d.StencilReadMask,
                    StencilWriteMask: d.StencilWriteMask,
                    FrontFace: d.FrontFace,
                    BackFace: d.BackFace,
                };
            }
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_VIEW_INSTANCING => {
                // `D3D12_OPTIONS3` reports no view instancing.
                if value!(D3D12_VIEW_INSTANCING_DESC).ViewInstanceCount != 0 {
                    return Err(E_INVALIDARG.into());
                }
            }
            // Amplification and mesh shaders, which `D3D12_OPTIONS7` reports absent: taken only
            // empty.
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_AS | D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_MS => {
                let code = value!(D3D12_SHADER_BYTECODE);
                if !code.pShaderBytecode.is_null() && code.BytecodeLength != 0 {
                    return Err(E_INVALIDARG.into());
                }
            }
            // Types that do not exist.
            _ => return Err(E_INVALIDARG.into()),
        }
    }
    // The program's root signature, borrowed for the call: the descriptions never release it.
    // SAFETY: a root signature pointer or null, as the API requires.
    let root = ManuallyDrop::new(
        (!root.is_null()).then(|| unsafe { ID3D12RootSignature::from_raw(root) }),
    );
    match (cs.pShaderBytecode.is_null(), g.VS.pShaderBytecode.is_null()) {
        (false, true) => Ok(Pipeline::Compute(D3D12_COMPUTE_PIPELINE_STATE_DESC {
            pRootSignature: root,
            CS: cs,
            NodeMask: g.NodeMask,
            CachedPSO: g.CachedPSO,
            Flags: g.Flags,
        })),
        (true, false) => {
            g.pRootSignature = root;
            Ok(Pipeline::Graphics(Box::new(g), topology_given))
        }
        _ => Err(E_INVALIDARG.into()),
    }
}
