//! The Rust expressions of the types a derive cannot write.
//!
//! Those are the types behind a checked constructor, and those of a level that adds no instruction, since
//! `Infallible` has no `Bake` for a derive to ask of it.

use core::convert::Infallible;

use databake::{Bake, CrateEnv, TokenStream, quote};

use crate::{DocumentedInstruction, LoadStoreInstruction, PreciseInstruction};

/// Written through `new`, so that a baked instruction the type does not hold fails to compile.
impl Bake for DocumentedInstruction {
    fn bake(&self, env: &CrateEnv) -> TokenStream {
        env.insert(env!("CARGO_CRATE_NAME"));
        let instruction = self.instruction().bake(env);
        quote!(wcontainers_processor_architecture_arm64::DocumentedInstruction::new(#instruction).unwrap())
    }
}

/// Written through `new`, so that a baked instruction the type does not hold fails to compile.
impl Bake for LoadStoreInstruction {
    fn bake(&self, env: &CrateEnv) -> TokenStream {
        env.insert(env!("CARGO_CRATE_NAME"));
        let instruction = self.instruction().bake(env);
        quote!(wcontainers_processor_architecture_arm64::LoadStoreInstruction::new(#instruction).unwrap())
    }
}

impl Bake for PreciseInstruction<Infallible> {
    fn bake(&self, env: &CrateEnv) -> TokenStream {
        env.insert(env!("CARGO_CRATE_NAME"));
        match self {
            Self::Documented(documented) => {
                let documented = documented.bake(env);
                quote!(wcontainers_processor_architecture_arm64::PreciseInstruction::Documented(#documented))
            }
            Self::LoadStore {
                instruction,
                choices,
            } => {
                let instruction = instruction.bake(env);
                let choices = choices.bake(env);
                quote!(wcontainers_processor_architecture_arm64::PreciseInstruction::LoadStore {
                    instruction: #instruction,
                    choices: #choices,
                })
            }
            Self::Brk => quote!(wcontainers_processor_architecture_arm64::PreciseInstruction::Brk),
            Self::Rdvl(reading) => {
                let reading = reading.bake(env);
                quote!(wcontainers_processor_architecture_arm64::PreciseInstruction::Rdvl(#reading))
            }
            Self::WaitForInterrupt(behavior) => {
                let behavior = behavior.bake(env);
                quote!(wcontainers_processor_architecture_arm64::PreciseInstruction::WaitForInterrupt(#behavior))
            }
            Self::SystemRegisterMove { direction, choices } => {
                let direction = direction.bake(env);
                let choices = choices.bake(env);
                quote!(wcontainers_processor_architecture_arm64::PreciseInstruction::SystemRegisterMove {
                    direction: #direction,
                    choices: #choices,
                })
            }
            Self::MrsCtrEl0 { cache_type } => {
                let cache_type = cache_type.bake(env);
                quote!(wcontainers_processor_architecture_arm64::PreciseInstruction::MrsCtrEl0 {
                    cache_type: #cache_type,
                })
            }
            Self::Unimplemented(instruction) => {
                let instruction = instruction.bake(env);
                quote!(wcontainers_processor_architecture_arm64::PreciseInstruction::Unimplemented(#instruction))
            }
            Self::LevelSpecific(never) => match *never {},
        }
    }
}
