//! Dispatches: the compute program converted by the NIR route, its resources bound as a draw's.

use wcontainers_gpu::{Backend, DispatchCall, DispatchGroups, Object, Program, SlowPath, StageSet};

use crate::context::{Context, end_if_lost};
use crate::draw::{self, Variant};
use crate::gallium::*;
use crate::resource::Resource;
use crate::state::{Stage, refuse, refused};
use crate::view::Images;

/// `pipe_context::launch_grid`.
pub unsafe extern "C" fn launch_grid<B: Backend>(
    pipe: *mut pipe_context,
    info: *const pipe_grid_info,
) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context and the grid.
        let (context, info) = unsafe { (Context::<B>::of(pipe), &*info) };
        if let Err(reason) = dispatch(context, info) {
            refuse(&reason);
        }
    })
}

fn dispatch<B: Backend>(context: &mut Context<B>, info: &pipe_grid_info) -> Result<(), String> {
    let shader = context.stages.get(Stage::Compute).shader;
    if shader.is_null() {
        return refused(format_args!("a dispatch without a compute program"));
    }
    if info.variable_shared_mem != 0 {
        return refused(format_args!(
            "workgroup memory sized at the dispatch (the screen reports none)"
        ));
    }
    let screen = context.screen();
    let mut guard = screen.backend();
    let backend = &mut *guard;
    let bound = context.stages.get(Stage::Compute);
    let (variant, swizzled) = Variant::sampling(bound);
    if swizzled {
        backend.slow_paths().hit(SlowPath::SwizzleInShader);
    }
    let programs = &mut context.programs;
    // SAFETY: a bound shader lives until Mesa unbinds and deletes it.
    let shader = unsafe { &*shader };
    let key = (shader.id, variant.clone());
    let missing = !programs.compute.contains_key(&key);
    let built = programs.built_for(backend, shader, &variant)?;
    if missing {
        let shader = built
            .shader
            .as_ref()
            .ok_or("a compute program without its shader")?;
        let program = backend
            .create_program(&Program::Compute(shader))
            .map_err(|e| format!("a compute program: {e:?}"))?;
        programs.compute.insert(key.clone(), program);
    }
    let built = programs.built_for(backend, shader, &variant)?;
    // Outside a pass, where images bound through a twin are copied.
    context
        .tracker
        .close(backend)
        .map_err(|e| format!("closing the pass for a dispatch: {e:?}"))?;
    let mut images = Images::default();
    let sets = draw::binding_sets(
        backend,
        &context.stages,
        &[(Stage::Compute, built)],
        &[],
        0,
        StageSet::COMPUTE,
        &mut images,
    )?;
    let groups = if info.indirect.is_null() {
        DispatchGroups::Direct(info.grid)
    } else {
        // SAFETY: an indirect grid names a live buffer.
        let resource = unsafe { Resource::<B>::of(info.indirect) };
        DispatchGroups::Indirect {
            args: resource
                .buffer()
                .ok_or("a dispatch's counts in a texture")?,
            offset: u64::from(info.indirect_offset),
        }
    };
    let result = backend.dispatch(&DispatchCall {
        program: &programs.compute[&key],
        bindings: std::array::from_fn(|set| sets[set].as_ref()),
        immediates: &[],
        groups,
    });
    for set in sets.into_iter().flatten() {
        backend.destroy(Object::BindingSet(set));
    }
    images.finish(backend);
    result.map_err(|e| format!("a dispatch: {e:?}"))?;
    end_if_lost(backend);
    Ok(())
}
