//! The Rust expressions of the types a derive cannot write.
//!
//! Those are the types behind a checked constructor or with private fields, and those of a level that adds no
//! instruction, since `Infallible` has no `Bake` for a derive to ask of it.

use alloc::format;
use alloc::string::String;
use core::convert::Infallible;

use databake::{Bake, BakeSize, CrateEnv, TokenStream, quote};

use crate::memory_tagging::TagAccessInstruction;
use crate::system_instruction::SystemFeatures;
use crate::{
    DocumentedInstruction, LoadStoreInstruction, PreciseInstruction, StructureInstruction,
};

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

/// Each feature of a system instruction with the name of its constant.
const NAMED_SYSTEM_FEATURES: [(&str, SystemFeatures); 14] = [
    (
        "POINTER_AUTHENTICATION",
        SystemFeatures::POINTER_AUTHENTICATION,
    ),
    ("FLAG_MANIPULATION", SystemFeatures::FLAG_MANIPULATION),
    ("FLAG_MANIPULATION_2", SystemFeatures::FLAG_MANIPULATION_2),
    (
        "SPECULATIVE_STORE_BYPASS_SAFE",
        SystemFeatures::SPECULATIVE_STORE_BYPASS_SAFE,
    ),
    (
        "DATA_INDEPENDENT_TIMING",
        SystemFeatures::DATA_INDEPENDENT_TIMING,
    ),
    ("MEMORY_TAGGING", SystemFeatures::MEMORY_TAGGING),
    ("PERSISTENCE", SystemFeatures::PERSISTENCE),
    ("DEEP_PERSISTENCE", SystemFeatures::DEEP_PERSISTENCE),
    (
        "PREDICTION_RESTRICTION",
        SystemFeatures::PREDICTION_RESTRICTION,
    ),
    (
        "PREDICTION_RESTRICTION_2",
        SystemFeatures::PREDICTION_RESTRICTION_2,
    ),
    (
        "OUTER_CACHE_MAINTENANCE",
        SystemFeatures::OUTER_CACHE_MAINTENANCE,
    ),
    (
        "GUARDED_CONTROL_STACK",
        SystemFeatures::GUARDED_CONTROL_STACK,
    ),
    (
        "INSTRUMENTATION_TRACE",
        SystemFeatures::INSTRUMENTATION_TRACE,
    ),
    (
        "IGNORED_TAG_CHECK_OVERRIDE",
        SystemFeatures::IGNORED_TAG_CHECK_OVERRIDE,
    ),
];

/// Written as `NONE` and the union of each named feature it holds, since its bits are private.
impl Bake for SystemFeatures {
    fn bake(&self, env: &CrateEnv) -> TokenStream {
        env.insert(env!("CARGO_CRATE_NAME"));
        let mut written = SystemFeatures::NONE;
        let mut baked = String::from(
            "wcontainers_processor_architecture_arm64::system_instruction::SystemFeatures::NONE",
        );
        for (name, feature) in NAMED_SYSTEM_FEATURES {
            if self.contains(feature) {
                written = written.union(feature);
                baked.push_str(&format!(
                    ".union(wcontainers_processor_architecture_arm64::system_instruction::SystemFeatures::{name})"
                ));
            }
        }
        assert!(written == *self, "every feature bit has a named constant");
        match baked.parse() {
            Ok(baked) => baked,
            Err(_) => unreachable!("a path and calls of a method on paths are Rust"),
        }
    }
}

/// Holds no reference.
impl BakeSize for SystemFeatures {
    fn borrows_size(&self) -> usize {
        0
    }
}

/// Written through `new`, so that a baked instruction the type does not hold fails to compile.
impl Bake for TagAccessInstruction {
    fn bake(&self, env: &CrateEnv) -> TokenStream {
        env.insert(env!("CARGO_CRATE_NAME"));
        let instruction = self.instruction().bake(env);
        quote!(wcontainers_processor_architecture_arm64::memory_tagging::TagAccessInstruction::new(#instruction).unwrap())
    }
}

/// Written through `new`, so that a baked instruction the type does not hold fails to compile.
impl Bake for StructureInstruction {
    fn bake(&self, env: &CrateEnv) -> TokenStream {
        env.insert(env!("CARGO_CRATE_NAME"));
        let instruction = self.instruction().bake(env);
        quote!(wcontainers_processor_architecture_arm64::StructureInstruction::new(#instruction).unwrap())
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
            Self::Scalable {
                instruction,
                operation,
                trapped,
            } => {
                let instruction = instruction.bake(env);
                let operation = operation.bake(env);
                let trapped = trapped.bake(env);
                quote!(wcontainers_processor_architecture_arm64::PreciseInstruction::Scalable {
                    instruction: #instruction,
                    operation: #operation,
                    trapped: #trapped,
                })
            }
            Self::ScalableAccess {
                instruction,
                choices,
            } => {
                let instruction = instruction.bake(env);
                let choices = choices.bake(env);
                quote!(wcontainers_processor_architecture_arm64::PreciseInstruction::ScalableAccess {
                    instruction: #instruction,
                    choices: #choices,
                })
            }
            Self::SystemInstruction(behavior) => {
                let behavior = behavior.bake(env);
                quote!(wcontainers_processor_architecture_arm64::PreciseInstruction::SystemInstruction(#behavior))
            }
            Self::ElementAccess {
                instruction,
                choices,
            } => {
                let instruction = instruction.bake(env);
                let choices = choices.bake(env);
                quote!(wcontainers_processor_architecture_arm64::PreciseInstruction::ElementAccess {
                    instruction: #instruction,
                    choices: #choices,
                })
            }
            Self::TagAccess {
                instruction,
                choices,
            } => {
                let instruction = instruction.bake(env);
                let choices = choices.bake(env);
                quote!(wcontainers_processor_architecture_arm64::PreciseInstruction::TagAccess {
                    instruction: #instruction,
                    choices: #choices,
                })
            }
            Self::Structure {
                instruction,
                choices,
            } => {
                let instruction = instruction.bake(env);
                let choices = choices.bake(env);
                quote!(wcontainers_processor_architecture_arm64::PreciseInstruction::Structure {
                    instruction: #instruction,
                    choices: #choices,
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
