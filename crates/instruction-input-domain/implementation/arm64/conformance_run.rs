wcontainers_instruction_input_domain_conformance::tests!(
    (
        wcontainers_instruction_input_domain_arm64::Arm64InputDomain,
        wcontainers_processor_architecture_arm64::Arm64::<
            wcontainers_processor_architecture_arm64::User,
        >::default()
    )
);
