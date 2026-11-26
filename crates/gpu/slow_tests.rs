//! Tests of the tables [`crate::slow`] declares.

mod tests {
    use crate::slow::*;
    use alloc::vec::Vec;

    #[test]
    fn all_lists_every_reason_in_order() {
        for (i, p) in SLOW_PATHS.iter().enumerate() {
            assert_eq!(*p as usize, i);
        }
        let mut names: Vec<_> = SLOW_PATH_REASONS.to_vec();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), SLOW_PATH_COUNT);
    }

    /// Every reason keeps the number it had when it was added, the place serialised caches read.
    #[test]
    fn reasons_keep_their_numbers() {
        const HELD: &str = "\
            FanConverted LoopConverted ReadBackWait PipelineMiss WriteSplitPass ReinterpretCopy \
            GeometryAsCompute GeometryAsReplay TessellationAsCompute StreamOutAsCompute \
            VertexStorageAsCompute RestartRewrite ProvokingRewrite DepthBiasInShader \
            BorderInShader LogicOpInShader FillModeConverted TexelBufferAsStorage StoragePacked \
            SamplerPairsExpanded ClipDepthRewrite VertexFormatConverted FormatConverted \
            TexelBufferAsTexture BaseVertexRebind IndirectReadBack IndirectCountExpanded \
            MappedUploadWhole ImmediatesSpilled MultiviewSerial MultiViewportSplit PointsAsQuads \
            F64Emulated PredicationWait DepthReadByDraw StencilFacesSplit MirrorOnceInShader \
            InstanceRateInShader OcclusionCountReplay TimestampSplitPass UnalignedBufferEdge \
            TargetsSplit FilterInShader StreamOutRepacked TargetReadPerPrimitive LayerSplit \
            VertexFetchZeroed BindingsBeyondLimit VertexPulledInShader ConstantsAsStorage \
            ConstantsAsTexture BlockDecoded HostCopyReadBack LodBiasInShader ViewportDepthFlip \
            VaryingsBeyondLimit AtomicsThroughBuffer NormAsInteger BlendInShader \
            SampleMaskInShader StencilCopiedByDraw CopyThroughBuffer DepthWrittenByDraw \
            HostCopyKept QueriesSegmented CopyWithinBuffer EarlyTestsInShader \
            EarlyTestsPerPrimitive MinLodClamp ViewportInShader StagingRefreshed \
            AttachmentsFitted IndexBufferTwin IndexTwinReadBack QueryResolveWait \
            PredicationIndirect IndexWidened AdjacencyDropped TextureInfoInHelpers \
            DrawBaseInHelpers DrawBaseFromArguments StorageFormatThroughBuffer \
            MultisampleCopyResolved AliasCopied AntialiasedLinesAsQuads UsageScopeSplit \
            UsageScopeCopied ClipDistanceInShader DepthClampInShader CullDistanceAsGeometry \
            PrimitiveIdAsGeometry RowsRepacked ResolveInStoredFormat ResolveByDraw \
            PatternClearCopied ConstantBufferGrown IntegerClearByDraw StencilWrittenByDraw \
            PresentConverted CaptureExpanded CopyWithinTexture PixelInvocationsCounted \
            SwizzleInShader AtomicExchangeLoop SequentialBuffersCopied IndirectFetchPastEnd \
            StorageOffsetInShader FloatBitsClearByDraw StreamOutAfterReplay FrameStretched \
            SharedThroughHost QuerySetSplitPass PackedTargetQuantised PackedTargetReadByBlend \
            StorageReadWriteThroughBuffer AliasedLinesAsQuads";
        let held: Vec<_> = HELD.split_whitespace().collect();
        assert!(held.len() <= SLOW_PATH_COUNT);
        for (i, name) in held.iter().enumerate() {
            assert_eq!(alloc::format!("{:?}", SLOW_PATHS[i]), *name, "reason {i}");
        }
    }
}
