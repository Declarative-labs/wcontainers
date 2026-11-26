//! The form a sample takes in a baked file, and the writing of that file.
//!
//! A baked file is `pub const SAMPLE: super::Sample = super::Sample { core: .., measurements: &[..] };`, where its
//! folder's module names `Sample` as the `BakedSample` of the folder's architecture and core identity.

use alloc::vec::Vec;

use wcontainers_observation_collector::CoreSample;
use wcontainers_processor_architecture::ProcessorArchitecture;

/// A sample as a constant holds it, its measurements in a static slice where the collector's sample has a `Vec`.
pub struct BakedSample<Architecture: ProcessorArchitecture + 'static, CoreIdentity> {
    pub core: CoreIdentity,
    /// Each instruction of the architecture, in its listed order, with the one behavior the core shows for it.
    pub measurements: &'static [(Architecture::Instruction, Architecture::PreciseInstruction)],
}

impl<Architecture: ProcessorArchitecture, CoreIdentity: Clone>
    BakedSample<Architecture, CoreIdentity>
{
    /// Returns the sample as the collector gives it.
    pub fn to_sample(&self) -> CoreSample<Architecture, CoreIdentity> {
        CoreSample {
            core: self.core.clone(),
            measurements: Vec::from(self.measurements),
        }
    }
}

#[cfg(feature = "bake")]
pub use writing::{BakeSampleError, bake_sample};

#[cfg(feature = "bake")]
mod writing {
    use alloc::string::String;

    use databake::{Bake, CrateEnv, quote};
    use wcontainers_observation_collector::CoreSample;
    use wcontainers_processor_architecture::ProcessorArchitecture;

    const COMMENT: &str = "// ";
    const LINE_END: char = '\n';

    #[derive(Debug)]
    pub enum BakeSampleError {
        /// The measurements do not hold each instruction once, in the order the architecture lists them.
        MeasurementsOutOfListedOrder {
            /// The position of the first measurement that differs from the listed instruction there.
            position: usize,
        },
        /// A label holds a line break, which would end its comment.
        LabelWithLineBreak { position: usize },
        /// A `Bake` implementation wrote tokens that are not Rust.
        BakedTokensUnparsed(syn::Error),
    }

    /// Returns the source of the baked file of a sample, each label a comment line above it.
    ///
    /// The source is laid out as rustfmt lays out code, by prettyplease: rustfmt itself leaves an item whole when a
    /// path is longer than its line, as the full paths of a sample are.
    pub fn bake_sample<Architecture, CoreIdentity>(
        sample: &CoreSample<Architecture, CoreIdentity>,
        labels: &[String],
    ) -> Result<String, BakeSampleError>
    where
        Architecture: ProcessorArchitecture,
        Architecture::Instruction: Bake,
        Architecture::PreciseInstruction: Bake,
        CoreIdentity: Bake,
    {
        let mut listed = Architecture::list_instructions();
        for (position, (instruction, _)) in sample.measurements.iter().enumerate() {
            if listed.next() != Some(*instruction) {
                return Err(BakeSampleError::MeasurementsOutOfListedOrder { position });
            }
        }
        if listed.next().is_some() {
            return Err(BakeSampleError::MeasurementsOutOfListedOrder {
                position: sample.measurements.len(),
            });
        }
        if let Some(position) = labels
            .iter()
            .position(|label| label.lines().nth(1).is_some())
        {
            return Err(BakeSampleError::LabelWithLineBreak { position });
        }
        let env = CrateEnv::default();
        let core = sample.core.bake(&env);
        let measurements = sample.measurements.as_slice().bake(&env);
        let constant = quote! {
            pub const SAMPLE: super::Sample = super::Sample {
                core: #core,
                measurements: #measurements,
            };
        };
        let file = syn::parse2(constant).map_err(BakeSampleError::BakedTokensUnparsed)?;
        let mut source = String::new();
        for label in labels {
            source.push_str(COMMENT);
            source.push_str(label);
            source.push(LINE_END);
        }
        source.push(LINE_END);
        source.push_str(&prettyplease::unparse(&file));
        Ok(source)
    }
}
