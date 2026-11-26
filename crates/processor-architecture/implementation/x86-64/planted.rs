//! Named mutations of the simulation that the generated states must catch, each with its family of forms and the
//! pass or check that planted it, which only the `planted-bugs` feature can turn on.

use iced_x86::Register;

use crate::behavior::UndefinedFlags;
use crate::evex::{Conversion, EvexForm, EvexOperation, IntegerOperation, Number, find_evex_form};
use crate::float::{FloatForm, FloatOperation, Precision, find_float_form};
use crate::integer::{
    BitChange, Condition, OperandPlace, Operation, Outcome, find_integer_form, sign_extend, subtract_with_borrow,
    to_accumulator, to_data, to_mask, to_result_flags, with_status,
};
use crate::vector::{Lane, LaneOperation, Signedness, VectorEncoding, find_vector_form, join, saturate, split, to_signed};
use crate::{Flags, Instruction};

/// Who planted a bug first.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origin {
    FirstPass,
    FirstCheck,
    SecondPass,
    SecondCheck,
    ThirdPass,
    ThirdCheck,
    FourthPass,
    FourthCheck,
    FifthPass,
    FifthCheck,
    /// The port onto the committed interfaces, for the VEX forms.
    Port,
    /// The sync that lists every form, for the behavior that raises #UD.
    WholeList,
    /// The second half of AVX-512's forms.
    EvexSecondHalf,
}

/// The forms a bug shows on.
#[derive(Clone, Copy)]
pub enum Family {
    /// The legacy integer forms whose operation the test accepts.
    Integer(fn(Operation) -> bool),
    /// The legacy SSE integer forms whose operation the test accepts.
    Vector(fn(LaneOperation) -> bool),
    /// The legacy SSE floating-point forms the test accepts.
    Float(fn(&FloatForm) -> bool),
    /// Every form with an operand in memory, PUSH's stack included.
    MemoryOperand,
    /// Every near jump form.
    Branch,
    /// The forms named.
    Forms(&'static [Instruction]),
    /// Every VEX form of a lane operation.
    VexVector,
    /// Every form a chip may lack.
    Optional,
    /// The EVEX forms of [`crate::evex`] the test accepts.
    Evex(fn(&EvexForm) -> bool),
    Every,
}

/// One planted bug.
#[derive(Clone, Copy)]
pub struct PlantedBug {
    pub bug: Bug,
    /// The family's name, as a person reads it.
    pub family_name: &'static str,
    pub family: Family,
    pub origin: Origin,
}

impl PlantedBug {
    /// Whether the form is one of the bug's family.
    pub fn contains(&self, instruction: Instruction) -> bool {
        match self.family {
            Family::Integer(test) => find_integer_form(instruction).is_some_and(|form| test(form.operation)),
            Family::Vector(test) => find_vector_form(instruction).is_some_and(|form| test(form.operation)),
            Family::Float(test) => find_float_form(instruction).is_some_and(test),
            Family::MemoryOperand => {
                instruction == Instruction::PushGprv50
                    || find_integer_form(instruction).is_some_and(|form| form.place != OperandPlace::Register)
                    || find_vector_form(instruction).is_some_and(|form| form.place != OperandPlace::Register)
                    || find_float_form(instruction).is_some_and(|form| form.place != OperandPlace::Register)
            }
            Family::Branch => crate::branch::find_branch_form(instruction).is_some(),
            Family::Forms(forms) => forms.contains(&instruction),
            Family::VexVector => find_vector_form(instruction).is_some_and(|form| form.encoding != VectorEncoding::Legacy),
            Family::Optional => crate::is_optional(instruction),
            Family::Evex(test) => find_evex_form(instruction).is_some_and(|form| test(&form)),
            Family::Every => true,
        }
    }
}

/// Every planted bug; the doc comment of each says what it does wrong.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bug {
    // First pass.
    /// ADD and ADC take AF from bit 4 of the sum.
    AddAuxiliaryFromSumBitFour,
    /// PUSH raises #GP, not #SS, at a non-canonical stack.
    PushGeneralProtectionAtNonCanonical,
    // First check.
    /// PF from every bit of the result, not its low byte.
    ParityOverWholeResult,
    /// A 32-bit register write keeps the upper half.
    ThirtyTwoBitWriteKeepsUpperHalf,
    /// A write to a read-only page is reported as a page that is not present.
    ReadOnlyWriteReportedNotPresent,
    /// A store that faults part-way reports the fault at the access's first byte.
    StoreFaultAtAccessStart,
    /// A store that faults part-way writes the bytes below the fault whatever the behavior says.
    StoreWritesBytesBelowFault,
    /// An instruction that faults keeps the registers it wrote.
    FaultKeepsRegisterWrites,
    // Second pass.
    /// A fetch past the canonical edge goes on.
    FetchCanonicalUnchecked,
    /// No access raises #AC.
    AlignmentCheckNever,
    /// The kernel's half reads as pages that are not present.
    KernelHalfNotPresent,
    /// The whole access is checked as canonical before its alignment.
    CanonicalBeforeAlignment,
    /// FS and GS add no base.
    SegmentBasesIgnored,
    // Second check.
    /// ADD takes AF from the carry into bit 5.
    AddAuxiliaryFromBitFive,
    /// ADD takes OF from the destination's sign alone.
    AddOverflowIgnoresSourceSign,
    /// ADC takes CF from the sum without the carry in.
    AddWithCarryCarryIgnoresCarryIn,
    /// ADC takes OF from the sum without the carry in.
    AddWithCarryOverflowIgnoresCarryIn,
    /// ADC reads AH to BH as the low byte.
    AddWithCarryHighByteReadAsLow,
    /// SBB adds the borrow.
    SubtractWithBorrowAddsBorrow,
    /// SBB takes CF without the borrow.
    SubtractWithBorrowCarryIgnoresBorrow,
    /// SUB takes OF from the source's sign.
    SubtractOverflowFromSourceSign,
    /// SUB zero-extends a 32-bit immediate to 64 bits.
    SubtractImmediateZeroExtended,
    /// A 32-bit displacement is zero-extended.
    DisplacementZeroExtended,
    /// CMP does not sign-extend an 8-bit immediate.
    CompareImmediateByteNotSignExtended,
    /// CMP writes its difference.
    CompareWritesDifference,
    /// A misaligned read raises no #AC.
    ReadWithoutAlignmentCheck,
    /// AND keeps CF.
    AndCarryKept,
    /// OR takes PF over the whole operands.
    OrParityOverWholeOperands,
    /// OR's 32-bit register write keeps the upper half.
    OrWriteKeepsUpperHalf,
    /// FS adds no base.
    FsBaseIgnored,
    /// XOR keeps OF.
    XorOverflowKept,
    /// XOR writes the bytes below a fault.
    XorWritesBytesBelowFault,
    /// TEST takes SF from bit 63 at every size.
    TestSignFromBitSixtyThree,
    /// INC sets CF from an all-ones operand.
    IncrementSetsCarry,
    /// INC sets OF only at 64 bits.
    IncrementOverflowOnlyAtSixtyFourBits,
    /// INC reports a write to a read-only page as a page that is not present.
    IncrementReadOnlyReportedNotPresent,
    /// DEC takes AF from bit 3.
    DecrementAuxiliaryFromBitThree,
    /// DEC stores a 16-bit result as 4 bytes.
    DecrementStoresFourBytes,
    /// NEG never sets OF.
    NegateOverflowNeverSet,
    /// NEG clears AF.
    NegateAuxiliaryCleared,
    /// NOT's 32-bit register write keeps the upper half.
    NotWriteKeepsUpperHalf,
    /// NOT writes the flags of its result.
    NotWritesResultFlags,
    /// SHL masks a 64-bit count to 5 bits.
    ShiftLeftQuadwordCountFiveBits,
    /// SHL masks an 8-bit count to 3 bits.
    ShiftLeftByteCountThreeBits,
    /// SHR's one-bit step clears OF.
    ShiftRightStepOverflowCleared,
    /// SHR by 0 clears CF.
    ShiftRightZeroCountClearsCarry,
    /// SAR of 8 bits drops the sign.
    ShiftRightArithmeticByteDropsSign,
    /// SAR's one-bit step sets OF from the sign.
    ShiftRightArithmeticOverflowFromSign,
    /// ROL masks a 64-bit count to 5 bits.
    RotateLeftQuadwordCountFiveBits,
    /// ROR's one-bit OF is the result's top bit XOR CF.
    RotateRightOverflowFromTopAndCarry,
    /// RCL of 8 bits steps its count modulo 8.
    RotateLeftThroughCarryByteModuloEight,
    /// RCR rotates no carry in.
    RotateRightThroughCarryCarryNotRotatedIn,
    /// RCR of 16 bits steps its count modulo 16.
    RotateRightThroughCarryWordModuloSixteen,
    /// MOV's 32-bit register write keeps the upper half.
    MoveWriteKeepsUpperHalf,
    /// MOV zero-extends a 32-bit immediate to 64 bits.
    MoveImmediateZeroExtended,
    /// MOV's 64-bit load reads 4 bytes.
    MoveQuadwordLoadReadsFourBytes,
    /// MOV reports a store's fault as a read.
    MoveStoreFaultReportedAsRead,
    /// The address-size prefix is ignored: full registers, no wrap at 4 GiB.
    AddressSizePrefixIgnored,
    /// LE ignores OF.
    LessOrEqualWithoutOverflow,
    /// CMOVcc reads no memory, so faults on none, when its condition is false.
    MoveIfFalseSkipsRead,
    /// CMOVcc writes nothing when its condition is false, keeping a 32-bit register's upper half.
    MoveIfFalseKeepsUpperHalf,
    /// L ignores OF.
    LessWithoutOverflow,
    /// SETcc stores 2 bytes.
    SetIfStoresTwoBytes,
    /// A read's fault is reported at the access's first byte.
    ReadFaultAtAccessStart,
    /// The index's scale is ignored.
    IndexScaleIgnored,
    /// A locked access's fault is reported as a read.
    LockedFaultReportedAsRead,
    // Third pass.
    /// An EIP-relative address adds a register.
    InstructionPointerRelativeAddsRegister,
    /// The access's first byte is not checked as canonical before its alignment.
    FirstByteCanonicalUnchecked,
    /// 16 bytes give #UD, not #GP(0).
    SixteenBytesInvalidOpcode,
    /// Bytes that cannot be fetched read as zeros.
    MissingBytesReadAsZeros,
    /// The privileged forms complete.
    PrivilegedFormsComplete,
    // Third check.
    /// ADD takes PF from the low 16 bits.
    AddParityFromLowSixteenBits,
    /// ADC's AF ignores the carry in.
    AddWithCarryAuxiliaryIgnoresCarryIn,
    /// SBB's OF ignores the borrow.
    SubtractWithBorrowOverflowIgnoresBorrow,
    /// AND takes SF from bit 7 at every size.
    AndSignFromBitSeven,
    /// CMP sets CF when equal too.
    CompareCarryWhenEqual,
    /// TEST keeps CF.
    TestCarryKept,
    /// INC takes AF from the carry out of bit 4.
    IncrementAuxiliaryFromBitFour,
    /// DEC takes OF from bit 63 at every size.
    DecrementOverflowFromBitSixtyThree,
    /// NEG sets CF for zero and clears it otherwise.
    NegateCarryWhenZero,
    /// SHL of 16 bits masks its count to 4 bits.
    ShiftLeftWordCountFourBits,
    /// SHR by one clears OF.
    ShiftRightByOneOverflowCleared,
    /// SAR of 16 bits shifts in zeros.
    ShiftRightArithmeticWordAsLogical,
    /// ROL of 32 bits masks its count to 6 bits.
    RotateLeftDoublewordCountSixBits,
    /// ROR takes CF from bit 0 of the result.
    RotateRightCarryFromBitZero,
    /// RCL of 16 bits steps its count modulo 16.
    RotateLeftThroughCarryWordModuloSixteen,
    /// RCR of 8 bits steps its count modulo 8.
    RotateRightThroughCarryByteModuloEight,
    /// CMOVNLE ignores ZF.
    MoveIfNotLessOrEqualIgnoresZero,
    /// SETBE ignores ZF.
    SetIfBelowOrEqualIgnoresZero,
    /// IMUL takes CF and OF from the unsigned product.
    MultiplySignedOverflowFromUnsignedProduct,
    /// IMUL r16, r/m16, imm8 does not sign-extend its immediate.
    MultiplySignedImmediateByteNotSignExtended,
    /// MOVZX from 16 bits reads 8.
    ZeroExtendWordReadsByte,
    /// MOVSX from 8 bits drops the sign.
    SignExtendByteDropsSign,
    /// MOVSXD zero-extends.
    SignExtendDoublewordZeroExtends,
    /// XCHG's second register keeps its upper half on a 32-bit write.
    ExchangeSecondKeepsUpperHalf,
    /// XADD swaps the sum and the old value.
    ExchangeAddSwapped,
    /// LOCK XADD writes its register despite the store's fault.
    ExchangeAddWritesDespiteFault,
    /// MUL of 8 bits writes the high half into DL.
    MultiplyWideByteHighIntoDl,
    /// IMUL with one operand sets CF and OF from a nonzero high half.
    MultiplyWideSignedOverflowFromNonzeroHigh,
    /// DIV truncates a quotient too wide, without #DE.
    DivideTooWideTruncated,
    /// IDIV's remainder takes the divisor's sign.
    DivideSignedRemainderTakesDivisorSign,
    /// CDQE zero-extends.
    SignExtendAccumulatorDoublewordZeroExtends,
    /// CBW takes the sign from bit 15, leaving AX as it was.
    SignExtendAccumulatorByteFromBitFifteen,
    /// CQO takes the sign from bit 31.
    SignIntoDataQuadwordFromBitThirtyOne,
    /// LAHF clears bit 1.
    LoadStatusIntoHighBitOneClear,
    /// SAHF clears OF.
    StoreHighIntoStatusClearsOverflow,
    /// CMC sets CF.
    ComplementCarrySets,
    /// NOP with memory reads it.
    NoOperationReadsMemory,
    /// XOR's 32-bit register write keeps the upper half.
    XorWriteKeepsUpperHalf,
    /// MOV's 16-bit register write clears the bits above.
    MoveWordWriteClearsUpperBits,
    /// SHLD of 32 bits masks its count to 4 bits.
    ShiftLeftDoubleDoublewordCountFourBits,
    /// SHRD by one clears OF.
    ShiftRightDoubleByOneOverflowCleared,
    /// BTC of 64 bits masks its offset to 5 bits.
    BitComplementQuadwordOffsetFiveBits,
    /// BTS takes CF after setting the bit.
    BitSetCarryAfterSet,
    /// BTR into memory takes its register offset unsigned.
    BitResetOffsetUnsigned,
    /// BT into memory moves the access by the offset in bytes.
    BitTestOffsetMovesInBytes,
    /// A store to a read-only page reports a page that is not present.
    StoreReadOnlyReportedNotPresent,
    /// A store that faults writes all its bytes.
    StoreFaultWritesAllBytes,
    /// #AC ignores 2-byte accesses.
    AlignmentCheckIgnoresTwoBytes,
    /// [RSP] or [RBP] through FS or GS still raises #SS.
    StackSegmentDespiteFsOrGs,
    /// A 64-bit displacement alone is zero-extended from 32 bits.
    DisplacementAloneZeroExtended,
    /// An EIP-relative address does not wrap at 4 GiB.
    InstructionPointerRelativeNotWrapped,
    /// 15 bytes are taken as too long.
    FifteenBytesTooLong,
    /// SWAPGS raises #UD, not #GP(0).
    SwapgsInvalidOpcode,
    /// PUSH lowers RSP despite the store's fault.
    PushLowersStackPointerDespiteFault,
    /// CMPXCHG of 32 bits on a mismatch keeps RAX's upper half.
    CompareExchangeMismatchKeepsUpperHalf,
    /// PADDD carries across lanes.
    AddDoublewordsCarryAcrossLanes,
    /// The FS override adds the GS base.
    FsOverrideAddsGsBase,
    // Fourth check.
    /// PADDSW saturates unsigned.
    AddSaturatingWordsUnsigned,
    /// PSUBUSB wraps instead of saturating at 0.
    SubtractSaturatingBytesWraps,
    /// PAVGB does not round up.
    AverageBytesNotRounded,
    /// PCMPGTW compares unsigned.
    CompareGreaterWordsUnsigned,
    /// PMULHW drops the sign extension.
    MultiplyHighWordsUnsigned,
    /// PSRAW by 16 or more gives 0, not the sign.
    ShiftRightArithmeticWordsWideCountZero,
    /// PSLLQ masks its count to 6 bits.
    ShiftLeftQuadwordsCountSixBits,
    /// PACKUSWB reads its words unsigned.
    PackUnsignedReadsUnsigned,
    /// PMADDWD saturates its pair sums instead of wrapping them.
    MultiplyAddWordsSaturates,
    /// PUNPCKHQDQ takes the low quadwords.
    UnpackHighQuadwordsTakesLow,
    /// PSADBW gives one sum of all 16 bytes.
    SumAbsoluteDifferencesWhole,
    /// POR from misaligned memory does not fault.
    OrMisalignedNoFault,
    /// PXOR from memory reads 8 bytes.
    XorReadsEightBytes,
    /// PADDQ carries across the quadwords.
    AddQuadwordsCarryAcross,
    /// Single-precision ADD swaps rounding up and down.
    AddSingleRoundingSwapped,
    /// Double precision detects tininess before rounding.
    DoubleTininessBeforeRounding,
    /// Single-precision DIV of a denormal by zero reports DE too.
    DivideSingleDenormalByZeroReportsDenormal,
    /// Double-precision SUB gives +0 for an exact cancellation under rounding down.
    SubtractDoubleCancellationPositiveZero,
    /// Single-precision SQRT of -0 gives +0.
    SquareRootSingleNegativeZeroPositive,
    /// Double-precision MAX of a quiet NaN raises no invalid.
    MaximumDoubleQuietNanNoInvalid,
    /// Single-precision MIN of two zeros gives the destination.
    MinimumSingleZerosGiveDestination,
    /// Double-precision ADD of two NaNs gives the source's.
    AddDoubleTwoNansGiveSource,
    /// Double-precision SQRT ignores DAZ.
    SquareRootDoubleIgnoresDenormalsAreZeros,
    /// Double-precision overflow rounding toward zero gives infinity.
    DoubleOverflowTowardZeroInfinity,
    /// ADDSS from memory reads 8 bytes.
    AddScalarSingleReadsEightBytes,
    /// MULPD writes its destination despite #XM.
    MultiplyPackedDoubleWritesDespiteFault,
    /// SUBSS from memory leaves MXCSR's flags unset on #XM.
    SubtractScalarSingleFaultLeavesFlags,
    /// DIVPD from misaligned memory does not fault.
    DividePackedDoubleMisalignedNoFault,
    /// Double precision rounds ties away from zero.
    DoubleTiesAwayFromZero,
    /// Single-precision unmasked overflow always sets PE.
    SingleUnmaskedOverflowAlwaysInexact,
    /// Single-precision MAX reports no DE for a denormal operand.
    MaximumSingleNoDenormalFlag,
    /// Single-precision SQRT of a negative number gives a positive default NaN.
    SquareRootSingleNegativeNanPositive,
    /// BSR of zero zeroes the destination.
    BitScanReverseZeroZeroes,
    /// BSF takes PF from the source's low byte.
    BitScanForwardParityFromSource,
    /// BSWAP of 64 bits swaps 32.
    ByteSwapQuadwordSwapsThirtyTwo,
    /// CMPXCHG of 8 bits takes its flags from the destination less the accumulator.
    CompareExchangeByteFlagsReversed,
    /// CMPXCHG r32 on a mismatch writes the destination back, clearing its upper half.
    CompareExchangeMismatchWritesBack,
    /// LEA adds the FS or GS base.
    LoadEffectiveAddressAddsSegmentBase,
    /// LEA of a 32-bit address does not wrap at 4 GiB.
    LoadEffectiveAddressNotWrapped,
    /// The length counts only legacy prefixes, the old rule.
    LengthCountsLegacyPrefixesOnly,
    // Fifth pass.
    /// PMAXSB compares unsigned.
    MaximumSignedBytesUnsigned,
    /// PCMPGTQ compares unsigned.
    CompareGreaterQuadwordsUnsigned,
    /// UNPCKHPS takes the low doublewords.
    UnpackHighDoublewordsTakesLow,
    /// PACKUSDW reads its doublewords unsigned.
    PackUnsignedDoublewordsReadsUnsigned,
    /// PSHUFB ignores the top bit of its index.
    ShuffleBytesIgnoresZeroingBit,
    /// PSIGNW keeps the destination where the source is zero.
    SignWordsKeepsForZero,
    /// PHSUBW subtracts each even lane from the odd one.
    HorizontalSubtractWordsReversed,
    /// PMADDUBSW reads the destination's bytes signed.
    MultiplyAddBytesBothSigned,
    /// PMULHRSW does not round.
    MultiplyHighRoundScaleNotRounded,
    /// PABSB saturates the lowest value to 0x7F.
    AbsoluteBytesSaturates,
    /// PMULDQ multiplies unsigned.
    MultiplyDoublewordsSignedAsUnsigned,
    /// PHADDSW wraps instead of saturating.
    HorizontalAddSaturatingWraps,
    /// PSHUFD selects the source's doublewords in reverse field order.
    ShuffleDoublewordsFieldsReversed,
    /// PALIGNR shifts by the immediate's low four bits alone.
    AlignRightCountFourBits,
    /// BLENDPS reads the immediate's bits from the top.
    BlendSinglesBitsReversed,
    /// PSRLDQ by 16 or more keeps the destination's top byte.
    ShiftBytesRightKeepsTopByte,
    /// A taken Jcc to a non-canonical target jumps.
    JumpToNonCanonical,
    /// JL ignores OF.
    JumpIfLessIgnoresOverflow,
    /// LOOP with ECX keeps RCX's upper half.
    LoopCountKeepsUpperHalf,
    /// LOOPNE jumps while ZF is set.
    LoopWhileNotZeroReversed,
    /// MOVUPS from misaligned memory raises #GP(0).
    MoveUnalignedFaults,
    /// MOVSHDUP takes the even singles.
    DuplicateOddSinglesTakesEven,
    /// MOVDDUP from memory reads 16 bytes.
    DuplicateLowDoubleReadsSixteenBytes,
    /// CMPcc reads four bits of its predicate.
    ComparePredicateFourBits,
    /// CMPLTPS raises no invalid for a quiet NaN.
    CompareLessQuiet,
    /// UCOMISS raises invalid for a quiet NaN.
    UnorderedCompareSignalsQuiet,
    /// COMISS of a NaN leaves PF clear.
    OrderedCompareUnorderedWithoutParity,
    /// HADDPS puts the source's sums low.
    HorizontalAddSourceFirst,
    /// ADDSUBPD adds in the even lanes and subtracts in the odd.
    AddSubtractReversed,
    /// CVTTPS2DQ and CVTTPD2DQ round under MXCSR instead of truncating.
    TruncatingConversionRounds,
    /// CVTPD2PS and CVTPD2DQ keep the destination's upper half.
    PackedConversionKeepsUpperHalf,
    /// MOVSS from memory keeps the destination's upper lanes.
    MoveScalarFromMemoryKeepsUpper,
    /// PMOVSXBW zero-extends.
    ExtendBytesSignedAsUnsigned,
    /// MOVHLPS takes the source's low quadword.
    MoveHighToLowTakesLow,
    /// POPCNT leaves CF as it was.
    PopulationCountKeepsCarry,
    /// PTEST sets CF from the destination and source's shared bits.
    TestCarryFromAnd,
    /// PHMINPOSUW takes the last index of a tie.
    MinimumWordPositionLastIndex,
    /// INSERTPS ignores its zero mask.
    InsertSingleIgnoresZeroMask,
    /// RDPMC raises #UD.
    ReadPerformanceCounterInvalidOpcode,
    // Fourth pass, made again from its descriptions.
    /// Round to nearest takes ties away from zero.
    NearestTiesAwayFromZero,
    /// FTZ is ignored.
    FlushToZeroIgnored,
    /// DAZ is ignored.
    DenormalsAreZerosIgnored,
    /// An unmasked overflow never sets PE.
    UnmaskedOverflowNeverInexact,
    /// MIN of two zeros gives the destination.
    MinimumOfZerosGivesDestination,
    /// PADDUSB saturates signed.
    AddSaturatingBytesSigned,
    /// PUNPCKLBW takes the source's byte first.
    UnpackLowBytesSourceFirst,
    /// The default NaN is positive.
    DefaultNanPositive,
    // The port.
    /// VEX.128 keeps the destination's upper half.
    VexNarrowKeepsUpperHalf,
    /// VEX.256 combines the first source's lower half into the destination's upper half.
    VexWideUpperFromLowerHalf,
    /// A VEX memory operand that is not aligned to 16 raises #GP(0), as a legacy one does.
    VexMisalignedFaults,
    /// VEX.256 reads 16 bytes of memory, the upper half zero.
    VexWideReadsSixteenBytes,
    /// The first source is the destination, as for a legacy form.
    VexFirstSourceIsDestination,
    // The whole list.
    /// A form the chip lacks raises #GP(0), not #UD.
    UndefinedRaisesGeneralProtection,
    // The second half of AVX-512.
    /// An element the opmask register leaves out under zeroing keeps the destination's value.
    EvexZeroingMerges,
    /// Exception suppression and embedded rounding still record the flags and raise #XM.
    EvexSuppressedExceptionsFlagged,
    /// A scalar form keeps the destination's bits beyond 128.
    EvexScalarKeepsUpperBits,
    /// A write to ZMM16 to ZMM31 goes to the register 16 below.
    EvexHighRegistersAliasLow,
    /// The opmask register is ignored: every element is computed.
    EvexMaskIgnored,
    /// Embedded rounding uses MXCSR's rounding mode.
    EvexEmbeddedRoundingIgnored,
    /// A broadcast source is read as a whole vector.
    EvexBroadcastReadsVector,
    /// An element the opmask register leaves out still faults.
    EvexMaskedFaultNotSuppressed,
    /// VPERMD and its kin take the index modulo half the element count.
    EvexPermuteIndexHalved,
    /// The two-table permutes always read the first table.
    EvexTwoTablesSelectorIgnored,
    /// The in-lane permutes and shuffles read the lowest lane for every lane.
    EvexLanesFromTheFirst,
    /// VPSHUFB ignores the control byte's zeroing bit.
    EvexShuffleBytesZeroBitIgnored,
    /// The unpacks take the second source's element first.
    EvexUnpackSourcesSwapped,
    /// VALIGND and VALIGNQ shift by the whole immediate, zero beyond both sources.
    EvexAlignCountUnreduced,
    /// The permutes and shuffles suppress a fault on an element the opmask leaves out.
    EvexPermuteFaultsSuppressed,
    /// VPDPBUSDS and VPDPWSSDS wrap rather than saturate.
    EvexDotProductWraps,
    /// The double shifts take their two sources the other way round.
    EvexShiftDoubleHalvesSwapped,
    /// VPCONFLICTD and VPCONFLICTQ count the element itself.
    EvexConflictIncludesSelf,
    /// VAESENC and VAESDEC skip MixColumns.
    EvexAesSkipsMixColumns,
    /// VPCLMULQDQ always takes the second source's low quadword.
    EvexCarryLessIgnoresBitFour,
    /// A form that writes an opmask register keeps the bits its opmask leaves out.
    EvexMaskDestinationMerges,
    /// The fused sum drops the bits of the addend or product it shifts out.
    EvexFusedStickyIgnored,
    /// A negative number converted to an unsigned integer wraps rather than being invalid.
    EvexUnsignedNegativeWraps,
    /// VCVTNEPS2BF16 rounds a denormal rather than reading it as zero.
    EvexBfloatKeepsDenormals,
    /// The prefetch hints and CLDEMOTE read their memory operand and fault where it does.
    HintsReadMemory,
    /// The EVEX compares ignore bit 4 of the predicate, which swaps whether a quiet NaN signals.
    EvexCompareSignalingBitIgnored,
    /// SEAMCALL, TDCALL, ENCLV, VMFUNC and the rest complete as no operation.
    RingThreeUndefinedFormsComplete,
    // Fifth check.
    /// SUB takes AF from bit 4 of the result.
    SubtractAuxiliaryFromResultBitFour,
    /// CMP takes OF from the destination's and difference's signs alone.
    CompareOverflowFromDestinationSign,
    /// ADC takes ZF from the sum not truncated to the operand size.
    AddWithCarryZeroFromUntruncatedSum,
    /// SBB of 32 bits takes SF from bit 63 of the untruncated difference.
    SubtractWithBorrowSignFromBitSixtyThree,
    /// OR keeps CF.
    OrCarryKept,
    /// NEG takes SF from bit 63 at every size.
    NegateSignFromBitSixtyThree,
    /// INC clears CF.
    IncrementClearsCarry,
    /// DEC sets CF when it borrows.
    DecrementSetsCarryAtZero,
    /// SHR of 32 bits masks its count to 6 bits.
    ShiftRightDoublewordCountSixBits,
    /// SAR of 64 bits masks its count to 5 bits.
    ShiftRightArithmeticQuadwordCountFiveBits,
    /// RCR of 64 bits masks its count to 5 bits.
    RotateRightThroughCarryQuadwordCountFiveBits,
    /// ROL of 8 bits masks its count to 3 bits.
    RotateLeftByteCountThreeBits,
    /// SHL by one takes OF from the old CF.
    ShiftLeftOneOverflowFromCarryIn,
    /// MOVSX from 16 bits zero-extends.
    SignExtendWordZeroExtends,
    /// MOVZX from 8 bits sign-extends.
    ZeroExtendByteSignExtends,
    /// IMUL of 32 bits never sets CF or OF.
    MultiplySignedDoublewordNeverOverflows,
    /// MUL of 64 bits swaps RAX and RDX.
    MultiplyWideQuadwordHalvesSwapped,
    /// IDIV raises #DE for the most negative quotient.
    DivideSignedMostNegativeQuotientFaults,
    /// XADD takes OF from the destination's sign alone.
    ExchangeAddOverflowIgnoresSourceSign,
    /// CMOVBE ignores CF.
    MoveIfBelowOrEqualIgnoresCarry,
    /// SETNLE ignores ZF.
    SetIfGreaterIgnoresZero,
    /// BT of 16 bits masks its register offset to 3 bits.
    BitTestWordOffsetThreeBits,
    /// SHLD of 64 bits masks its count to 5 bits.
    ShiftLeftDoubleQuadwordCountFiveBits,
    /// BSR counts the index from the top.
    BitScanReverseFromTop,
    /// POPCNT of 64 bits counts the low 32 bits.
    PopulationCountLowHalfOnly,
    /// LEA's 32-bit write keeps the upper half.
    LoadEffectiveAddressWriteKeepsUpperHalf,
    /// SUB writes an AH to BH result into the low byte.
    SubtractHighByteWrittenLow,
    /// a read of an unmapped page is reported as a present page.
    ReadNotPresentReportedPresent,
    /// a fetch's page fault is reported as a read.
    FetchFaultReportedAsRead,
    /// SHL into memory writes RFLAGS despite the store's fault.
    ShiftStoreFlagsDespiteFault,
    /// XCHG with memory writes the register despite the store's fault.
    ExchangeRegisterWrittenDespiteFault,
    /// #AC checks an 8-byte access for 4-byte alignment.
    AlignmentCheckEightBytesAtFour,
    /// a non-canonical address based on RBP raises #GP, not #SS.
    StackSegmentOnlyForRsp,
    /// MOVAPD from misaligned memory does not fault.
    MoveAlignedDoubleMisalignedNoFault,
    /// MOVLPD from memory reads 16 bytes.
    MoveLowDoubleReadsSixteenBytes,
    /// SUBSD from memory not aligned to 16 raises #GP(0).
    FloatScalarMisalignedFaults,
    /// PSUBSW saturates unsigned.
    SubtractSaturatingWordsUnsigned,
    /// PMULLW gives the high halves.
    MultiplyLowWordsGivesHigh,
    /// PSRLW masks its count to 4 bits.
    ShiftRightLogicalWordsCountFourBits,
    /// PSRAD by 32 or more gives 0, not the sign.
    ShiftRightArithmeticDoublewordsWideZero,
    /// PACKSSDW saturates unsigned.
    PackSignedDoublewordsSaturatesUnsigned,
    /// PMINUW compares signed.
    MinimumWordsUnsignedAsSigned,
    /// PUNPCKLWD takes the high words.
    UnpackLowWordsTakesHigh,
    /// PSHUFLW shuffles the high words too.
    ShuffleLowWordsShufflesHigh,
    /// PMULUDQ multiplies the odd doublewords.
    MultiplyUnsignedDoublewordsOddLanes,
    /// PSLLDQ masks its count to 4 bits.
    ShiftBytesLeftCountFourBits,
    /// PABSD saturates the lowest value to 0x7FFF_FFFF.
    AbsoluteDoublewordsSaturates,
    /// PHADDD puts the source's sums low.
    HorizontalAddDoublewordsSourceLow,
    /// PMOVZXBD sign-extends.
    ExtendBytesToDoublewordsZeroAsSigned,
    /// PMINSD compares unsigned.
    MinimumDoublewordsSignedAsUnsigned,
    /// PBLENDW reads its immediate's bits from the top.
    BlendWordsBitsReversed,
    /// PCMPGTB compares unsigned.
    CompareGreaterBytesUnsigned,
    /// SHUFPS takes its low pair from the source.
    ShuffleSinglesOperandsSwapped,
    /// MOVLHPS takes the source's high quadword.
    MoveLowToHighTakesHigh,
    /// PANDN inverts the source.
    AndNotReversed,
    /// CVTPS2DQ truncates instead of rounding under MXCSR.
    ConversionToIntegerTruncates,
    /// CVTSD2SS rounds to nearest under every mode.
    DoubleToSingleRoundsToNearest,
    /// CVTDQ2PS rounds to nearest under every mode.
    IntegerToSingleRoundsToNearest,
    /// MINSD with a NaN gives the destination.
    MinimumDoubleNanGivesDestination,
    /// CMPLEPD is false for equal operands.
    CompareLessOrEqualExcludesEqual,
    /// COMISD of a NaN leaves ZF clear.
    OrderedCompareDoubleNanClearsZero,
    /// UCOMISD of a signaling NaN raises no invalid.
    UnorderedCompareDoubleSignalingNoInvalid,
    /// HSUBPD subtracts the low lane from the high one.
    HorizontalSubtractDoubleReversed,
    /// CVTSS2SD of a signaling NaN leaves it signaling.
    SingleToDoubleKeepsSignaling,
    /// double-precision masked underflow is flagged for an exact tiny result.
    MaskedUnderflowOnExactTiny,
    /// CVTPS2PD of a denormal raises no denormal.
    SingleToDoubleDenormalUnflagged,
    /// double-precision SQRT never sets PE.
    SquareRootDoubleNeverInexact,
    /// double-precision 0/0 reports divide-by-zero, not invalid.
    DivideDoubleZeroByZeroReportsDivideByZero,
    /// JMP rel8 zero-extends its displacement.
    JumpRelativeByteZeroExtended,
    /// JRCXZ tests ECX.
    JumpIfCountZeroTestsLowHalf,
    /// JBE ignores ZF.
    JumpIfBelowOrEqualIgnoresZero,
    /// LOOPE ignores ZF.
    LoopWhileZeroIgnoresZero,
    /// JMP r64 truncates its target to 32 bits.
    JumpIndirectTruncated,
    /// HLT raises #UD, not #GP(0).
    HaltInvalidOpcode,
    /// RDMSR completes at ring 3.
    ReadModelSpecificRegisterCompletes,
}

const fn row(bug: Bug, family_name: &'static str, family: Family, origin: Origin) -> PlantedBug {
    PlantedBug { bug, family_name, family, origin }
}

const fn integer(test: fn(Operation) -> bool) -> Family {
    Family::Integer(test)
}

use Origin::{
    EvexSecondHalf, FifthCheck, FifthPass, FirstCheck, FirstPass, FourthCheck, FourthPass, Port, SecondCheck, SecondPass,
    ThirdCheck, ThirdPass, WholeList,
};

/// Every planted bug of every pass and check, with its family.
pub static PLANTED_BUGS: &[PlantedBug] = &[
    row(Bug::AddAuxiliaryFromSumBitFour, "ADD, ADC", integer(|o| matches!(o, Operation::Add | Operation::AddWithCarry)), FirstPass),
    row(Bug::PushGeneralProtectionAtNonCanonical, "PUSH", Family::Forms(&[Instruction::PushGprv50]), FirstPass),
    row(Bug::ParityOverWholeResult, "integer result flags", Family::Every, FirstCheck),
    row(Bug::ThirtyTwoBitWriteKeepsUpperHalf, "register write", Family::Every, FirstCheck),
    row(Bug::ReadOnlyWriteReportedNotPresent, "page fault", Family::MemoryOperand, FirstCheck),
    row(Bug::StoreFaultAtAccessStart, "store", Family::MemoryOperand, FirstCheck),
    row(Bug::StoreWritesBytesBelowFault, "store", Family::MemoryOperand, FirstCheck),
    row(Bug::FaultKeepsRegisterWrites, "fault", Family::Every, FirstCheck),
    row(Bug::FetchCanonicalUnchecked, "fetch", Family::Every, SecondPass),
    row(Bug::AlignmentCheckNever, "alignment check", Family::MemoryOperand, SecondPass),
    row(Bug::KernelHalfNotPresent, "page fault", Family::MemoryOperand, SecondPass),
    row(Bug::CanonicalBeforeAlignment, "fault order", Family::MemoryOperand, SecondPass),
    row(Bug::SegmentBasesIgnored, "addressing", Family::MemoryOperand, SecondPass),
    row(Bug::SixteenBytesInvalidOpcode, "fetch", Family::Every, SecondPass),
    row(Bug::AddAuxiliaryFromBitFive, "ADD", integer(|o| o == Operation::Add), SecondCheck),
    row(Bug::AddOverflowIgnoresSourceSign, "ADD", integer(|o| o == Operation::Add), SecondCheck),
    row(Bug::AddWithCarryCarryIgnoresCarryIn, "ADC", integer(|o| o == Operation::AddWithCarry), SecondCheck),
    row(Bug::AddWithCarryOverflowIgnoresCarryIn, "ADC", integer(|o| o == Operation::AddWithCarry), SecondCheck),
    row(Bug::AddWithCarryHighByteReadAsLow, "ADC", integer(|o| o == Operation::AddWithCarry), SecondCheck),
    row(Bug::SubtractWithBorrowAddsBorrow, "SBB", integer(|o| o == Operation::SubtractWithBorrow), SecondCheck),
    row(Bug::SubtractWithBorrowCarryIgnoresBorrow, "SBB", integer(|o| o == Operation::SubtractWithBorrow), SecondCheck),
    row(Bug::SubtractOverflowFromSourceSign, "SUB", integer(|o| o == Operation::Subtract), SecondCheck),
    row(Bug::SubtractImmediateZeroExtended, "SUB", integer(|o| o == Operation::Subtract), SecondCheck),
    row(Bug::DisplacementZeroExtended, "addressing", Family::MemoryOperand, SecondCheck),
    row(Bug::CompareImmediateByteNotSignExtended, "CMP", integer(|o| o == Operation::Compare), SecondCheck),
    row(Bug::CompareWritesDifference, "CMP", integer(|o| o == Operation::Compare), SecondCheck),
    row(Bug::ReadWithoutAlignmentCheck, "alignment check", Family::MemoryOperand, SecondCheck),
    row(Bug::AndCarryKept, "AND", integer(|o| o == Operation::And), SecondCheck),
    row(Bug::OrParityOverWholeOperands, "OR", integer(|o| o == Operation::Or), SecondCheck),
    row(Bug::OrWriteKeepsUpperHalf, "OR", integer(|o| o == Operation::Or), SecondCheck),
    row(Bug::FsBaseIgnored, "addressing", Family::MemoryOperand, SecondCheck),
    row(Bug::XorOverflowKept, "XOR", integer(|o| o == Operation::Xor), SecondCheck),
    row(Bug::XorWritesBytesBelowFault, "XOR", integer(|o| o == Operation::Xor), SecondCheck),
    row(Bug::TestSignFromBitSixtyThree, "TEST", integer(|o| o == Operation::Test), SecondCheck),
    row(Bug::IncrementSetsCarry, "INC", integer(|o| o == Operation::Increment), SecondCheck),
    row(Bug::IncrementOverflowOnlyAtSixtyFourBits, "INC", integer(|o| o == Operation::Increment), SecondCheck),
    row(Bug::IncrementReadOnlyReportedNotPresent, "INC", integer(|o| o == Operation::Increment), SecondCheck),
    row(Bug::DecrementAuxiliaryFromBitThree, "DEC", integer(|o| o == Operation::Decrement), SecondCheck),
    row(Bug::DecrementStoresFourBytes, "DEC", integer(|o| o == Operation::Decrement), SecondCheck),
    row(Bug::NegateOverflowNeverSet, "NEG", integer(|o| o == Operation::Negate), SecondCheck),
    row(Bug::NegateAuxiliaryCleared, "NEG", integer(|o| o == Operation::Negate), SecondCheck),
    row(Bug::NotWriteKeepsUpperHalf, "NOT", integer(|o| o == Operation::Not), SecondCheck),
    row(Bug::NotWritesResultFlags, "NOT", integer(|o| o == Operation::Not), SecondCheck),
    row(Bug::ShiftLeftQuadwordCountFiveBits, "SHL", integer(|o| o == Operation::ShiftLeft), SecondCheck),
    row(Bug::ShiftLeftByteCountThreeBits, "SHL", integer(|o| o == Operation::ShiftLeft), SecondCheck),
    row(Bug::ShiftRightStepOverflowCleared, "SHR", integer(|o| o == Operation::ShiftRightLogical), SecondCheck),
    row(Bug::ShiftRightZeroCountClearsCarry, "SHR", integer(|o| o == Operation::ShiftRightLogical), SecondCheck),
    row(Bug::ShiftRightArithmeticByteDropsSign, "SAR", integer(|o| o == Operation::ShiftRightArithmetic), SecondCheck),
    row(Bug::ShiftRightArithmeticOverflowFromSign, "SAR", integer(|o| o == Operation::ShiftRightArithmetic), SecondCheck),
    row(Bug::RotateLeftQuadwordCountFiveBits, "ROL", integer(|o| o == Operation::RotateLeft), SecondCheck),
    row(Bug::RotateRightOverflowFromTopAndCarry, "ROR", integer(|o| o == Operation::RotateRight), SecondCheck),
    row(Bug::RotateLeftThroughCarryByteModuloEight, "RCL", integer(|o| o == Operation::RotateLeftThroughCarry), SecondCheck),
    row(Bug::RotateRightThroughCarryCarryNotRotatedIn, "RCR", integer(|o| o == Operation::RotateRightThroughCarry), SecondCheck),
    row(Bug::RotateRightThroughCarryWordModuloSixteen, "RCR", integer(|o| o == Operation::RotateRightThroughCarry), SecondCheck),
    row(Bug::MoveWriteKeepsUpperHalf, "MOV", integer(|o| o == Operation::Move), SecondCheck),
    row(Bug::MoveImmediateZeroExtended, "MOV", integer(|o| o == Operation::Move), SecondCheck),
    row(Bug::MoveQuadwordLoadReadsFourBytes, "MOV", integer(|o| o == Operation::Move), SecondCheck),
    row(Bug::MoveStoreFaultReportedAsRead, "MOV", integer(|o| o == Operation::Move), SecondCheck),
    row(Bug::AddressSizePrefixIgnored, "addressing", Family::MemoryOperand, SecondCheck),
    row(Bug::LessOrEqualWithoutOverflow, "CMOVLE, SETLE", integer(|o| matches!(o, Operation::MoveIf(_) | Operation::SetIf(_))), SecondCheck),
    row(Bug::MoveIfFalseSkipsRead, "CMOVcc", integer(|o| matches!(o, Operation::MoveIf(_))), SecondCheck),
    row(Bug::MoveIfFalseKeepsUpperHalf, "CMOVcc", integer(|o| matches!(o, Operation::MoveIf(_))), SecondCheck),
    row(Bug::LessWithoutOverflow, "CMOVL, SETL", integer(|o| matches!(o, Operation::MoveIf(_) | Operation::SetIf(_))), SecondCheck),
    row(Bug::SetIfStoresTwoBytes, "SETcc", integer(|o| matches!(o, Operation::SetIf(_))), SecondCheck),
    row(Bug::ReadFaultAtAccessStart, "page fault", Family::MemoryOperand, SecondCheck),
    row(Bug::IndexScaleIgnored, "addressing", Family::MemoryOperand, SecondCheck),
    row(Bug::LockedFaultReportedAsRead, "locked store", integer(|_| true), SecondCheck),
    row(Bug::InstructionPointerRelativeAddsRegister, "addressing", Family::MemoryOperand, ThirdPass),
    row(Bug::FirstByteCanonicalUnchecked, "fault order", Family::MemoryOperand, ThirdPass),
    row(Bug::MissingBytesReadAsZeros, "fetch", Family::Every, ThirdPass),
    row(Bug::PrivilegedFormsComplete, "privileged", Family::Every, ThirdPass),
    row(Bug::AddParityFromLowSixteenBits, "ADD", integer(|o| o == Operation::Add), ThirdCheck),
    row(Bug::AddWithCarryAuxiliaryIgnoresCarryIn, "ADC", integer(|o| o == Operation::AddWithCarry), ThirdCheck),
    row(Bug::SubtractWithBorrowOverflowIgnoresBorrow, "SBB", integer(|o| o == Operation::SubtractWithBorrow), ThirdCheck),
    row(Bug::AndSignFromBitSeven, "AND", integer(|o| o == Operation::And), ThirdCheck),
    row(Bug::CompareCarryWhenEqual, "CMP", integer(|o| o == Operation::Compare), ThirdCheck),
    row(Bug::TestCarryKept, "TEST", integer(|o| o == Operation::Test), ThirdCheck),
    row(Bug::IncrementAuxiliaryFromBitFour, "INC", integer(|o| o == Operation::Increment), ThirdCheck),
    row(Bug::DecrementOverflowFromBitSixtyThree, "DEC", integer(|o| o == Operation::Decrement), ThirdCheck),
    row(Bug::NegateCarryWhenZero, "NEG", integer(|o| o == Operation::Negate), ThirdCheck),
    row(Bug::ShiftLeftWordCountFourBits, "SHL", integer(|o| o == Operation::ShiftLeft), ThirdCheck),
    row(Bug::ShiftRightByOneOverflowCleared, "SHR", integer(|o| o == Operation::ShiftRightLogical), ThirdCheck),
    row(Bug::ShiftRightArithmeticWordAsLogical, "SAR", integer(|o| o == Operation::ShiftRightArithmetic), ThirdCheck),
    row(Bug::RotateLeftDoublewordCountSixBits, "ROL", integer(|o| o == Operation::RotateLeft), ThirdCheck),
    row(Bug::RotateRightCarryFromBitZero, "ROR", integer(|o| o == Operation::RotateRight), ThirdCheck),
    row(Bug::RotateLeftThroughCarryWordModuloSixteen, "RCL", integer(|o| o == Operation::RotateLeftThroughCarry), ThirdCheck),
    row(Bug::RotateRightThroughCarryByteModuloEight, "RCR", integer(|o| o == Operation::RotateRightThroughCarry), ThirdCheck),
    row(Bug::MoveIfNotLessOrEqualIgnoresZero, "CMOVNLE", integer(|o| o == Operation::MoveIf(Condition::NotLessOrEqual)), ThirdCheck),
    row(Bug::SetIfBelowOrEqualIgnoresZero, "SETBE", integer(|o| o == Operation::SetIf(Condition::BelowOrEqual)), ThirdCheck),
    row(Bug::MultiplySignedOverflowFromUnsignedProduct, "IMUL", integer(|o| o == Operation::MultiplySigned), ThirdCheck),
    row(Bug::MultiplySignedImmediateByteNotSignExtended, "IMUL", integer(|o| o == Operation::MultiplySigned), ThirdCheck),
    row(Bug::ZeroExtendWordReadsByte, "MOVZX", integer(|o| o == Operation::ZeroExtend), ThirdCheck),
    row(Bug::SignExtendByteDropsSign, "MOVSX", integer(|o| o == Operation::SignExtend), ThirdCheck),
    row(Bug::SignExtendDoublewordZeroExtends, "MOVSXD", integer(|o| o == Operation::SignExtend), ThirdCheck),
    row(Bug::ExchangeSecondKeepsUpperHalf, "XCHG", integer(|o| o == Operation::Exchange), ThirdCheck),
    row(Bug::ExchangeAddSwapped, "XADD", integer(|o| o == Operation::ExchangeAdd), ThirdCheck),
    row(Bug::ExchangeAddWritesDespiteFault, "XADD", integer(|o| o == Operation::ExchangeAdd), ThirdCheck),
    row(Bug::MultiplyWideByteHighIntoDl, "MUL", integer(|o| matches!(o, Operation::MultiplyWide { .. })), ThirdCheck),
    row(Bug::MultiplyWideSignedOverflowFromNonzeroHigh, "IMUL", integer(|o| matches!(o, Operation::MultiplyWide { .. })), ThirdCheck),
    row(Bug::DivideTooWideTruncated, "DIV", integer(|o| matches!(o, Operation::DivideWide { .. })), ThirdCheck),
    row(Bug::DivideSignedRemainderTakesDivisorSign, "IDIV", integer(|o| matches!(o, Operation::DivideWide { .. })), ThirdCheck),
    row(Bug::SignExtendAccumulatorDoublewordZeroExtends, "CDQE", integer(|o| o == Operation::SignExtendAccumulator), ThirdCheck),
    row(Bug::SignExtendAccumulatorByteFromBitFifteen, "CBW", integer(|o| o == Operation::SignExtendAccumulator), ThirdCheck),
    row(Bug::SignIntoDataQuadwordFromBitThirtyOne, "CQO", integer(|o| o == Operation::SignIntoData), ThirdCheck),
    row(Bug::LoadStatusIntoHighBitOneClear, "LAHF", integer(|o| o == Operation::LoadStatusIntoHigh), ThirdCheck),
    row(Bug::StoreHighIntoStatusClearsOverflow, "SAHF", integer(|o| o == Operation::StoreHighIntoStatus), ThirdCheck),
    row(Bug::ComplementCarrySets, "CMC", integer(|o| o == Operation::ComplementCarry), ThirdCheck),
    row(Bug::NoOperationReadsMemory, "NOP", integer(|o| o == Operation::NoOperation), ThirdCheck),
    row(Bug::XorWriteKeepsUpperHalf, "XOR", integer(|o| o == Operation::Xor), ThirdCheck),
    row(Bug::MoveWordWriteClearsUpperBits, "MOV", integer(|o| o == Operation::Move), ThirdCheck),
    row(Bug::ShiftLeftDoubleDoublewordCountFourBits, "SHLD", integer(|o| o == Operation::ShiftLeftDouble), ThirdCheck),
    row(Bug::ShiftRightDoubleByOneOverflowCleared, "SHRD", integer(|o| o == Operation::ShiftRightDouble), ThirdCheck),
    row(Bug::BitComplementQuadwordOffsetFiveBits, "BTC", integer(|o| o == Operation::BitTest(BitChange::Complement)), ThirdCheck),
    row(Bug::BitSetCarryAfterSet, "BTS", integer(|o| o == Operation::BitTest(BitChange::Set)), ThirdCheck),
    row(Bug::BitResetOffsetUnsigned, "BTR", integer(|o| o == Operation::BitTest(BitChange::Reset)), ThirdCheck),
    row(Bug::BitTestOffsetMovesInBytes, "BT", integer(|o| o == Operation::BitTest(BitChange::Keep)), ThirdCheck),
    row(Bug::StoreReadOnlyReportedNotPresent, "store", Family::MemoryOperand, ThirdCheck),
    row(Bug::StoreFaultWritesAllBytes, "store", Family::MemoryOperand, ThirdCheck),
    row(Bug::AlignmentCheckIgnoresTwoBytes, "alignment check", Family::MemoryOperand, ThirdCheck),
    row(Bug::StackSegmentDespiteFsOrGs, "addressing", Family::MemoryOperand, ThirdCheck),
    row(Bug::DisplacementAloneZeroExtended, "addressing", Family::MemoryOperand, ThirdCheck),
    row(Bug::InstructionPointerRelativeNotWrapped, "addressing", Family::MemoryOperand, ThirdCheck),
    row(Bug::FifteenBytesTooLong, "fetch", Family::Every, ThirdCheck),
    row(Bug::SwapgsInvalidOpcode, "SWAPGS", Family::Forms(&[Instruction::Swapgs]), ThirdCheck),
    row(Bug::PushLowersStackPointerDespiteFault, "PUSH", Family::Forms(&[Instruction::PushGprv50]), ThirdCheck),
    row(Bug::CompareExchangeMismatchKeepsUpperHalf, "CMPXCHG", integer(|o| o == Operation::CompareExchange), ThirdCheck),
    row(Bug::AddDoublewordsCarryAcrossLanes, "PADDD", Family::Vector(|o| o == LaneOperation::Add(Lane::Doubleword)), ThirdCheck),
    row(Bug::FsOverrideAddsGsBase, "addressing", Family::MemoryOperand, ThirdCheck),
    row(Bug::AddSaturatingWordsUnsigned, "PADDSW", Family::Vector(|o| o == LaneOperation::AddSaturating(Lane::Word, Signedness::Signed)), FourthCheck),
    row(Bug::SubtractSaturatingBytesWraps, "PSUBUSB", Family::Vector(|o| o == LaneOperation::SubtractSaturating(Lane::Byte, Signedness::Unsigned)), FourthCheck),
    row(Bug::AverageBytesNotRounded, "PAVGB", Family::Vector(|o| o == LaneOperation::Average(Lane::Byte)), FourthCheck),
    row(Bug::CompareGreaterWordsUnsigned, "PCMPGTW", Family::Vector(|o| o == LaneOperation::CompareGreater(Lane::Word)), FourthCheck),
    row(Bug::MultiplyHighWordsUnsigned, "PMULHW", Family::Vector(|o| o == LaneOperation::MultiplyHighWords(Signedness::Signed)), FourthCheck),
    row(Bug::ShiftRightArithmeticWordsWideCountZero, "PSRAW", Family::Vector(|o| o == LaneOperation::ShiftRightArithmetic(Lane::Word)), FourthCheck),
    row(Bug::ShiftLeftQuadwordsCountSixBits, "PSLLQ", Family::Vector(|o| o == LaneOperation::ShiftLeft(Lane::Quadword)), FourthCheck),
    row(Bug::PackUnsignedReadsUnsigned, "PACKUSWB", Family::Vector(|o| o == LaneOperation::Pack(Lane::Word, Signedness::Unsigned)), FourthCheck),
    row(Bug::MultiplyAddWordsSaturates, "PMADDWD", Family::Vector(|o| o == LaneOperation::MultiplyAddWords), FourthCheck),
    row(Bug::UnpackHighQuadwordsTakesLow, "PUNPCKHQDQ", Family::Vector(|o| o == LaneOperation::UnpackHigh(Lane::Quadword)), FourthCheck),
    row(Bug::SumAbsoluteDifferencesWhole, "PSADBW", Family::Vector(|o| o == LaneOperation::SumAbsoluteDifferences), FourthCheck),
    row(Bug::OrMisalignedNoFault, "POR", Family::Vector(|o| o == LaneOperation::Or), FourthCheck),
    row(Bug::XorReadsEightBytes, "PXOR", Family::Vector(|o| o == LaneOperation::Xor), FourthCheck),
    row(Bug::AddQuadwordsCarryAcross, "PADDQ", Family::Vector(|o| o == LaneOperation::Add(Lane::Quadword)), FourthCheck),
    row(Bug::AddSingleRoundingSwapped, "ADDPS, ADDSS", Family::Float(|f| f.operation == FloatOperation::Add && f.precision == Precision::Single), FourthCheck),
    row(Bug::DoubleTininessBeforeRounding, "double precision", Family::Float(|f| f.precision == Precision::Double), FourthCheck),
    row(Bug::DivideSingleDenormalByZeroReportsDenormal, "DIVPS, DIVSS", Family::Float(|f| f.operation == FloatOperation::Divide && f.precision == Precision::Single), FourthCheck),
    row(Bug::SubtractDoubleCancellationPositiveZero, "SUBPD, SUBSD", Family::Float(|f| f.operation == FloatOperation::Subtract && f.precision == Precision::Double), FourthCheck),
    row(Bug::SquareRootSingleNegativeZeroPositive, "SQRTPS, SQRTSS", Family::Float(|f| f.operation == FloatOperation::SquareRoot && f.precision == Precision::Single), FourthCheck),
    row(Bug::MaximumDoubleQuietNanNoInvalid, "MAXPD, MAXSD", Family::Float(|f| f.operation == FloatOperation::Maximum && f.precision == Precision::Double), FourthCheck),
    row(Bug::MinimumSingleZerosGiveDestination, "MINPS, MINSS", Family::Float(|f| f.operation == FloatOperation::Minimum && f.precision == Precision::Single), FourthCheck),
    row(Bug::AddDoubleTwoNansGiveSource, "ADDPD, ADDSD", Family::Float(|f| f.operation == FloatOperation::Add && f.precision == Precision::Double), FourthCheck),
    row(Bug::SquareRootDoubleIgnoresDenormalsAreZeros, "SQRTPD, SQRTSD", Family::Float(|f| f.operation == FloatOperation::SquareRoot && f.precision == Precision::Double), FourthCheck),
    row(Bug::DoubleOverflowTowardZeroInfinity, "double precision", Family::Float(|f| f.precision == Precision::Double), FourthCheck),
    row(Bug::AddScalarSingleReadsEightBytes, "ADDSS", Family::Forms(&[Instruction::AddssXmmssMemss]), FourthCheck),
    row(Bug::MultiplyPackedDoubleWritesDespiteFault, "MULPD", Family::Forms(&[Instruction::MulpdXmmpdXmmpd]), FourthCheck),
    row(Bug::SubtractScalarSingleFaultLeavesFlags, "SUBSS", Family::Forms(&[Instruction::SubssXmmssMemss]), FourthCheck),
    row(Bug::DividePackedDoubleMisalignedNoFault, "DIVPD", Family::Forms(&[Instruction::DivpdXmmpdMempd]), FourthCheck),
    row(Bug::DoubleTiesAwayFromZero, "double precision", Family::Float(|f| f.precision == Precision::Double), FourthCheck),
    row(Bug::SingleUnmaskedOverflowAlwaysInexact, "single precision", Family::Float(|f| f.precision == Precision::Single), FourthCheck),
    row(Bug::MaximumSingleNoDenormalFlag, "MAXPS, MAXSS", Family::Float(|f| f.operation == FloatOperation::Maximum && f.precision == Precision::Single), FourthCheck),
    row(Bug::SquareRootSingleNegativeNanPositive, "SQRTPS, SQRTSS", Family::Float(|f| f.operation == FloatOperation::SquareRoot && f.precision == Precision::Single), FourthCheck),
    row(Bug::BitScanReverseZeroZeroes, "BSR", integer(|o| o == Operation::BitScanReverse), FourthCheck),
    row(Bug::BitScanForwardParityFromSource, "BSF", integer(|o| o == Operation::BitScanForward), FourthCheck),
    row(Bug::ByteSwapQuadwordSwapsThirtyTwo, "BSWAP", integer(|o| o == Operation::ByteSwap), FourthCheck),
    row(Bug::CompareExchangeByteFlagsReversed, "CMPXCHG", integer(|o| o == Operation::CompareExchange), FourthCheck),
    row(Bug::CompareExchangeMismatchWritesBack, "CMPXCHG", integer(|o| o == Operation::CompareExchange), FourthCheck),
    row(Bug::LoadEffectiveAddressAddsSegmentBase, "LEA", integer(|o| o == Operation::LoadEffectiveAddress), FourthCheck),
    row(Bug::LoadEffectiveAddressNotWrapped, "LEA", integer(|o| o == Operation::LoadEffectiveAddress), FourthCheck),
    row(Bug::LengthCountsLegacyPrefixesOnly, "fetch", Family::Every, FourthCheck),
    row(Bug::NearestTiesAwayFromZero, "floating point", Family::Float(|_| true), FourthPass),
    row(Bug::FlushToZeroIgnored, "floating point", Family::Float(|_| true), FourthPass),
    row(Bug::DenormalsAreZerosIgnored, "floating point", Family::Float(|_| true), FourthPass),
    row(Bug::UnmaskedOverflowNeverInexact, "floating point", Family::Float(|_| true), FourthPass),
    row(Bug::MinimumOfZerosGivesDestination, "MIN", Family::Float(|f| f.operation == FloatOperation::Minimum), FourthPass),
    row(Bug::AddSaturatingBytesSigned, "PADDUSB", Family::Vector(|o| o == LaneOperation::AddSaturating(Lane::Byte, Signedness::Unsigned)), FourthPass),
    row(Bug::UnpackLowBytesSourceFirst, "PUNPCKLBW", Family::Vector(|o| o == LaneOperation::UnpackLow(Lane::Byte)), FourthPass),
    row(Bug::DefaultNanPositive, "floating point", Family::Float(|_| true), FourthPass),
    row(Bug::MaximumSignedBytesUnsigned, "PMAXSB", Family::Vector(|o| o == LaneOperation::Maximum(Lane::Byte, Signedness::Signed)), FifthPass),
    row(Bug::CompareGreaterQuadwordsUnsigned, "PCMPGTQ", Family::Vector(|o| o == LaneOperation::CompareGreater(Lane::Quadword)), FifthPass),
    row(Bug::UnpackHighDoublewordsTakesLow, "UNPCKHPS, PUNPCKHDQ", Family::Vector(|o| o == LaneOperation::UnpackHigh(Lane::Doubleword)), FifthPass),
    row(Bug::ShuffleBytesIgnoresZeroingBit, "PSHUFB", Family::Vector(|o| o == LaneOperation::ShuffleBytes), FifthPass),
    row(Bug::SignWordsKeepsForZero, "PSIGNW", Family::Vector(|o| o == LaneOperation::Sign(Lane::Word)), FifthPass),
    row(Bug::HorizontalSubtractWordsReversed, "PHSUBW", Family::Vector(|o| o == LaneOperation::HorizontalSubtract(Lane::Word)), FifthPass),
    row(Bug::MultiplyAddBytesBothSigned, "PMADDUBSW", Family::Vector(|o| o == LaneOperation::MultiplyAddBytes), FifthPass),
    row(Bug::MultiplyHighRoundScaleNotRounded, "PMULHRSW", Family::Vector(|o| o == LaneOperation::MultiplyHighRoundScale), FifthPass),
    row(Bug::AbsoluteBytesSaturates, "PABSB", Family::Vector(|o| o == LaneOperation::Absolute(Lane::Byte)), FifthPass),
    row(Bug::MultiplyDoublewordsSignedAsUnsigned, "PMULDQ", Family::Vector(|o| o == LaneOperation::MultiplyDoublewords(Signedness::Signed)), FifthPass),
    row(Bug::HorizontalAddSaturatingWraps, "PHADDSW", Family::Vector(|o| o == LaneOperation::HorizontalAddSaturating), FifthPass),
    row(Bug::ShuffleDoublewordsFieldsReversed, "PSHUFD", Family::Vector(|o| o == LaneOperation::ShuffleDoublewords), FifthPass),
    row(Bug::AlignRightCountFourBits, "PALIGNR", Family::Vector(|o| o == LaneOperation::AlignRight), FifthPass),
    row(Bug::BlendSinglesBitsReversed, "BLENDPS", Family::Vector(|o| o == LaneOperation::Blend(Lane::Doubleword)), FifthPass),
    row(Bug::ShiftBytesRightKeepsTopByte, "PSRLDQ", Family::Vector(|o| o == LaneOperation::ShiftBytesRight), FifthPass),
    row(Bug::JumpToNonCanonical, "Jcc, JMP", Family::Branch, FifthPass),
    row(Bug::JumpIfLessIgnoresOverflow, "JL", Family::Branch, FifthPass),
    row(Bug::LoopCountKeepsUpperHalf, "LOOP", Family::Branch, FifthPass),
    row(Bug::LoopWhileNotZeroReversed, "LOOPNE", Family::Branch, FifthPass),
    row(Bug::ReadPerformanceCounterInvalidOpcode, "RDPMC", Family::Forms(&[Instruction::Rdpmc]), FifthPass),
    row(Bug::MoveUnalignedFaults, "MOVUPS", Family::Forms(&[Instruction::MovupsXmmpsMemps]), FifthPass),
    row(Bug::DuplicateOddSinglesTakesEven, "MOVSHDUP", Family::Vector(|o| o == LaneOperation::DuplicateOddSingles), FifthPass),
    row(Bug::DuplicateLowDoubleReadsSixteenBytes, "MOVDDUP", Family::Forms(&[Instruction::MovddupXmmdqMemq]), FifthPass),
    row(Bug::ComparePredicateFourBits, "CMPcc", Family::Float(|f| f.operation == FloatOperation::Compare), FifthPass),
    row(Bug::CompareLessQuiet, "CMPcc", Family::Float(|f| f.operation == FloatOperation::Compare), FifthPass),
    row(Bug::UnorderedCompareSignalsQuiet, "UCOMIS", Family::Float(|f| f.operation == FloatOperation::OrderedCompare { signaling: false }), FifthPass),
    row(Bug::OrderedCompareUnorderedWithoutParity, "COMIS, UCOMIS", Family::Float(|f| f.operation.writes_flags()), FifthPass),
    row(Bug::HorizontalAddSourceFirst, "HADDPS", Family::Float(|f| f.operation == FloatOperation::HorizontalAdd), FifthPass),
    row(Bug::AddSubtractReversed, "ADDSUBPD", Family::Float(|f| f.operation == FloatOperation::AddSubtract), FifthPass),
    row(Bug::TruncatingConversionRounds, "CVTT", Family::Float(|f| f.operation == FloatOperation::Convert(crate::float::Conversion::FloatToInteger { truncate: true })), FifthPass),
    row(Bug::PackedConversionKeepsUpperHalf, "CVTPD2PS, CVTPD2DQ", Family::Float(|f| matches!(f.operation, FloatOperation::Convert(_))), FifthPass),
    row(Bug::MoveScalarFromMemoryKeepsUpper, "MOVSS", Family::Forms(&[Instruction::MovssXmmdqMemss]), FifthPass),
    row(Bug::ExtendBytesSignedAsUnsigned, "PMOVSXBW", Family::Vector(|o| o == LaneOperation::Extend { from: Lane::Byte, to: Lane::Word, signedness: Signedness::Signed }), FifthPass),
    row(Bug::MoveHighToLowTakesLow, "MOVHLPS", Family::Vector(|o| o == LaneOperation::MoveHighToLow), FifthPass),
    row(Bug::PopulationCountKeepsCarry, "POPCNT", integer(|o| o == Operation::PopulationCount), FifthPass),
    row(Bug::TestCarryFromAnd, "PTEST", Family::Vector(|o| o == LaneOperation::Test), FifthPass),
    row(Bug::MinimumWordPositionLastIndex, "PHMINPOSUW", Family::Vector(|o| o == LaneOperation::MinimumWordPosition), FifthPass),
    row(Bug::InsertSingleIgnoresZeroMask, "INSERTPS", Family::Vector(|o| matches!(o, LaneOperation::InsertSingle { .. })), FifthPass),
    row(Bug::PackUnsignedDoublewordsReadsUnsigned, "PACKUSDW", Family::Vector(|o| o == LaneOperation::Pack(Lane::Doubleword, Signedness::Unsigned)), FifthPass),
    row(Bug::SubtractAuxiliaryFromResultBitFour, "SubGprvGprv29", Family::Forms(&[Instruction::SubGprvGprv29]), FifthCheck),
    row(Bug::CompareOverflowFromDestinationSign, "CmpGprvImmb", Family::Forms(&[Instruction::CmpGprvImmb]), FifthCheck),
    row(Bug::AddWithCarryZeroFromUntruncatedSum, "AdcGpr8Gpr810", Family::Forms(&[Instruction::AdcGpr8Gpr810]), FifthCheck),
    row(Bug::SubtractWithBorrowSignFromBitSixtyThree, "SbbGprvImmb", Family::Forms(&[Instruction::SbbGprvImmb]), FifthCheck),
    row(Bug::OrCarryKept, "OrGprvMemv", Family::Forms(&[Instruction::OrGprvMemv]), FifthCheck),
    row(Bug::NegateSignFromBitSixtyThree, "NegGpr8", Family::Forms(&[Instruction::NegGpr8]), FifthCheck),
    row(Bug::IncrementClearsCarry, "IncMemv", Family::Forms(&[Instruction::IncMemv]), FifthCheck),
    row(Bug::DecrementSetsCarryAtZero, "DecGprvFfr1", Family::Forms(&[Instruction::DecGprvFfr1]), FifthCheck),
    row(Bug::ShiftRightDoublewordCountSixBits, "ShrGprvCl", Family::Forms(&[Instruction::ShrGprvCl]), FifthCheck),
    row(Bug::ShiftRightArithmeticQuadwordCountFiveBits, "SarGprvImmb", Family::Forms(&[Instruction::SarGprvImmb]), FifthCheck),
    row(Bug::RotateRightThroughCarryQuadwordCountFiveBits, "RcrGprvCl", Family::Forms(&[Instruction::RcrGprvCl]), FifthCheck),
    row(Bug::RotateLeftByteCountThreeBits, "RolGpr8Cl", Family::Forms(&[Instruction::RolGpr8Cl]), FifthCheck),
    row(Bug::ShiftLeftOneOverflowFromCarryIn, "ShlGprvOneD1r4", Family::Forms(&[Instruction::ShlGprvOneD1r4]), FifthCheck),
    row(Bug::SignExtendWordZeroExtends, "MovsxGprvGpr16", Family::Forms(&[Instruction::MovsxGprvGpr16]), FifthCheck),
    row(Bug::ZeroExtendByteSignExtends, "MovzxGprvMemb", Family::Forms(&[Instruction::MovzxGprvMemb]), FifthCheck),
    row(Bug::MultiplySignedDoublewordNeverOverflows, "ImulGprvGprvImmz", Family::Forms(&[Instruction::ImulGprvGprvImmz]), FifthCheck),
    row(Bug::MultiplyWideQuadwordHalvesSwapped, "MulGprv", Family::Forms(&[Instruction::MulGprv]), FifthCheck),
    row(Bug::DivideSignedMostNegativeQuotientFaults, "IdivGpr8", Family::Forms(&[Instruction::IdivGpr8]), FifthCheck),
    row(Bug::ExchangeAddOverflowIgnoresSourceSign, "XaddGpr8Gpr8", Family::Forms(&[Instruction::XaddGpr8Gpr8]), FifthCheck),
    row(Bug::MoveIfBelowOrEqualIgnoresCarry, "CmovbeGprvMemv", Family::Forms(&[Instruction::CmovbeGprvMemv]), FifthCheck),
    row(Bug::SetIfGreaterIgnoresZero, "SetnleGpr8", Family::Forms(&[Instruction::SetnleGpr8]), FifthCheck),
    row(Bug::BitTestWordOffsetThreeBits, "BtGprvGprv", Family::Forms(&[Instruction::BtGprvGprv]), FifthCheck),
    row(Bug::ShiftLeftDoubleQuadwordCountFiveBits, "ShldGprvGprvImmb", Family::Forms(&[Instruction::ShldGprvGprvImmb]), FifthCheck),
    row(Bug::BitScanReverseFromTop, "BsrGprvMemv", Family::Forms(&[Instruction::BsrGprvMemv]), FifthCheck),
    row(Bug::PopulationCountLowHalfOnly, "PopcntGprvGprv", Family::Forms(&[Instruction::PopcntGprvGprv]), FifthCheck),
    row(Bug::LoadEffectiveAddressWriteKeepsUpperHalf, "LeaGprvAgen", Family::Forms(&[Instruction::LeaGprvAgen]), FifthCheck),
    row(Bug::SubtractHighByteWrittenLow, "SubGpr8Gpr828", Family::Forms(&[Instruction::SubGpr8Gpr828]), FifthCheck),
    row(Bug::ReadNotPresentReportedPresent, "AddGprvMemv", Family::Forms(&[Instruction::AddGprvMemv]), FifthCheck),
    row(Bug::FetchFaultReportedAsRead, "AddGpr8Gpr800", Family::Forms(&[Instruction::AddGpr8Gpr800]), FifthCheck),
    row(Bug::ShiftStoreFlagsDespiteFault, "ShlMemvClD3r4", Family::Forms(&[Instruction::ShlMemvClD3r4]), FifthCheck),
    row(Bug::ExchangeRegisterWrittenDespiteFault, "XchgMemvGprv", Family::Forms(&[Instruction::XchgMemvGprv]), FifthCheck),
    row(Bug::AlignmentCheckEightBytesAtFour, "MovGprvMemv", Family::Forms(&[Instruction::MovGprvMemv]), FifthCheck),
    row(Bug::StackSegmentOnlyForRsp, "MovMemvGprv", Family::Forms(&[Instruction::MovMemvGprv]), FifthCheck),
    row(Bug::MoveAlignedDoubleMisalignedNoFault, "MovapdXmmpdMempd", Family::Forms(&[Instruction::MovapdXmmpdMempd]), FifthCheck),
    row(Bug::MoveLowDoubleReadsSixteenBytes, "MovlpdXmmsdMemq", Family::Forms(&[Instruction::MovlpdXmmsdMemq]), FifthCheck),
    row(Bug::FloatScalarMisalignedFaults, "SubsdXmmsdMemsd", Family::Forms(&[Instruction::SubsdXmmsdMemsd]), FifthCheck),
    row(Bug::SubtractSaturatingWordsUnsigned, "PsubswXmmdqXmmdq", Family::Forms(&[Instruction::PsubswXmmdqXmmdq]), FifthCheck),
    row(Bug::MultiplyLowWordsGivesHigh, "PmullwXmmdqXmmdq", Family::Forms(&[Instruction::PmullwXmmdqXmmdq]), FifthCheck),
    row(Bug::ShiftRightLogicalWordsCountFourBits, "PsrlwXmmdqXmmdq", Family::Forms(&[Instruction::PsrlwXmmdqXmmdq]), FifthCheck),
    row(Bug::ShiftRightArithmeticDoublewordsWideZero, "PsradXmmdqImmb", Family::Forms(&[Instruction::PsradXmmdqImmb]), FifthCheck),
    row(Bug::PackSignedDoublewordsSaturatesUnsigned, "PackssdwXmmdqXmmdq", Family::Forms(&[Instruction::PackssdwXmmdqXmmdq]), FifthCheck),
    row(Bug::MinimumWordsUnsignedAsSigned, "PminuwXmmdqXmmdq", Family::Forms(&[Instruction::PminuwXmmdqXmmdq]), FifthCheck),
    row(Bug::UnpackLowWordsTakesHigh, "PunpcklwdXmmdqXmmq", Family::Forms(&[Instruction::PunpcklwdXmmdqXmmq]), FifthCheck),
    row(Bug::ShuffleLowWordsShufflesHigh, "PshuflwXmmdqXmmdqImmb", Family::Forms(&[Instruction::PshuflwXmmdqXmmdqImmb]), FifthCheck),
    row(Bug::MultiplyUnsignedDoublewordsOddLanes, "PmuludqXmmdqXmmdq", Family::Forms(&[Instruction::PmuludqXmmdqXmmdq]), FifthCheck),
    row(Bug::ShiftBytesLeftCountFourBits, "PslldqXmmdqImmb", Family::Forms(&[Instruction::PslldqXmmdqImmb]), FifthCheck),
    row(Bug::AbsoluteDoublewordsSaturates, "PabsdXmmdqXmmdq", Family::Forms(&[Instruction::PabsdXmmdqXmmdq]), FifthCheck),
    row(Bug::HorizontalAddDoublewordsSourceLow, "PhadddXmmdqXmmdq", Family::Forms(&[Instruction::PhadddXmmdqXmmdq]), FifthCheck),
    row(Bug::ExtendBytesToDoublewordsZeroAsSigned, "PmovzxbdXmmdqXmmd", Family::Forms(&[Instruction::PmovzxbdXmmdqXmmd]), FifthCheck),
    row(Bug::MinimumDoublewordsSignedAsUnsigned, "PminsdXmmdqXmmdq", Family::Forms(&[Instruction::PminsdXmmdqXmmdq]), FifthCheck),
    row(Bug::BlendWordsBitsReversed, "PblendwXmmdqXmmdqImmb", Family::Forms(&[Instruction::PblendwXmmdqXmmdqImmb]), FifthCheck),
    row(Bug::CompareGreaterBytesUnsigned, "PcmpgtbXmmdqXmmdq", Family::Forms(&[Instruction::PcmpgtbXmmdqXmmdq]), FifthCheck),
    row(Bug::ShuffleSinglesOperandsSwapped, "ShufpsXmmpsXmmpsImmb", Family::Forms(&[Instruction::ShufpsXmmpsXmmpsImmb]), FifthCheck),
    row(Bug::MoveLowToHighTakesHigh, "MovlhpsXmmqXmmq", Family::Forms(&[Instruction::MovlhpsXmmqXmmq]), FifthCheck),
    row(Bug::AndNotReversed, "PandnXmmdqXmmdq", Family::Forms(&[Instruction::PandnXmmdqXmmdq]), FifthCheck),
    row(Bug::ConversionToIntegerTruncates, "Cvtps2dqXmmdqXmmps", Family::Forms(&[Instruction::Cvtps2dqXmmdqXmmps]), FifthCheck),
    row(Bug::DoubleToSingleRoundsToNearest, "Cvtsd2ssXmmssXmmsd", Family::Forms(&[Instruction::Cvtsd2ssXmmssXmmsd]), FifthCheck),
    row(Bug::IntegerToSingleRoundsToNearest, "Cvtdq2psXmmpsXmmdq", Family::Forms(&[Instruction::Cvtdq2psXmmpsXmmdq]), FifthCheck),
    row(Bug::MinimumDoubleNanGivesDestination, "MinsdXmmsdXmmsd", Family::Forms(&[Instruction::MinsdXmmsdXmmsd]), FifthCheck),
    row(Bug::CompareLessOrEqualExcludesEqual, "CmppdXmmpdXmmpdImmb", Family::Forms(&[Instruction::CmppdXmmpdXmmpdImmb]), FifthCheck),
    row(Bug::OrderedCompareDoubleNanClearsZero, "ComisdXmmsdMemsd", Family::Forms(&[Instruction::ComisdXmmsdMemsd]), FifthCheck),
    row(Bug::UnorderedCompareDoubleSignalingNoInvalid, "UcomisdXmmsdXmmsd", Family::Forms(&[Instruction::UcomisdXmmsdXmmsd]), FifthCheck),
    row(Bug::HorizontalSubtractDoubleReversed, "HsubpdXmmpdXmmpd", Family::Forms(&[Instruction::HsubpdXmmpdXmmpd]), FifthCheck),
    row(Bug::SingleToDoubleKeepsSignaling, "Cvtss2sdXmmsdXmmss", Family::Forms(&[Instruction::Cvtss2sdXmmsdXmmss]), FifthCheck),
    row(Bug::MaskedUnderflowOnExactTiny, "MulsdXmmsdXmmsd", Family::Forms(&[Instruction::MulsdXmmsdXmmsd]), FifthCheck),
    row(Bug::SingleToDoubleDenormalUnflagged, "Cvtps2pdXmmpdXmmq", Family::Forms(&[Instruction::Cvtps2pdXmmpdXmmq]), FifthCheck),
    row(Bug::SquareRootDoubleNeverInexact, "SqrtpdXmmpdXmmpd", Family::Forms(&[Instruction::SqrtpdXmmpdXmmpd]), FifthCheck),
    row(Bug::DivideDoubleZeroByZeroReportsDivideByZero, "DivpdXmmpdXmmpd", Family::Forms(&[Instruction::DivpdXmmpdXmmpd]), FifthCheck),
    row(Bug::JumpRelativeByteZeroExtended, "JmpRelbrb", Family::Forms(&[Instruction::JmpRelbrb]), FifthCheck),
    row(Bug::JumpIfCountZeroTestsLowHalf, "JrcxzRelbrb", Family::Forms(&[Instruction::JrcxzRelbrb]), FifthCheck),
    row(Bug::JumpIfBelowOrEqualIgnoresZero, "JbeRelbrd", Family::Forms(&[Instruction::JbeRelbrd]), FifthCheck),
    row(Bug::LoopWhileZeroIgnoresZero, "LoopeRelbrb", Family::Forms(&[Instruction::LoopeRelbrb]), FifthCheck),
    row(Bug::JumpIndirectTruncated, "JmpGprv", Family::Forms(&[Instruction::JmpGprv]), FifthCheck),
    row(Bug::HaltInvalidOpcode, "Hlt", Family::Forms(&[Instruction::Hlt]), FifthCheck),
    row(Bug::ReadModelSpecificRegisterCompletes, "Rdmsr", Family::Forms(&[Instruction::Rdmsr]), FifthCheck),
    row(Bug::VexNarrowKeepsUpperHalf, "VEX lane operations", Family::VexVector, Port),
    row(Bug::VexWideUpperFromLowerHalf, "VEX lane operations", Family::VexVector, Port),
    row(Bug::VexMisalignedFaults, "VEX lane operations", Family::VexVector, Port),
    row(Bug::VexWideReadsSixteenBytes, "VEX lane operations", Family::VexVector, Port),
    row(Bug::VexFirstSourceIsDestination, "VEX lane operations", Family::VexVector, Port),
    row(Bug::UndefinedRaisesGeneralProtection, "forms a chip may lack", Family::Optional, WholeList),
    row(Bug::EvexZeroingMerges, "EVEX forms", Family::Evex(|_| true), EvexSecondHalf),
    row(Bug::EvexSuppressedExceptionsFlagged, "EVEX floating-point register forms", Family::Evex(is_float_register_form), EvexSecondHalf),
    row(Bug::EvexScalarKeepsUpperBits, "EVEX scalar forms", Family::Evex(|form| form.elements() == 1), EvexSecondHalf),
    row(Bug::EvexHighRegistersAliasLow, "EVEX forms that write a vector", Family::Evex(|form| !form.writes_opmask() && !matches!(form.operation, EvexOperation::OrderedCompare { .. })), EvexSecondHalf),
    row(Bug::EvexMaskIgnored, "EVEX forms", Family::Evex(|_| true), EvexSecondHalf),
    row(Bug::EvexEmbeddedRoundingIgnored, "EVEX forms with embedded rounding", Family::Evex(|form| form.code.op_code().can_use_rounding_control()), EvexSecondHalf),
    row(Bug::EvexBroadcastReadsVector, "EVEX forms that broadcast", Family::Evex(|form| form.place != OperandPlace::Register && form.code.op_code().can_broadcast()), EvexSecondHalf),
    row(Bug::EvexMaskedFaultNotSuppressed, "EVEX forms with memory that suppress faults", Family::Evex(|form| form.place != OperandPlace::Register && form.suppresses_faults()), EvexSecondHalf),
    row(Bug::EvexPermuteIndexHalved, "VPERMB to VPERMPD", Family::Evex(|form| is_integer(form, IntegerOperation::Permute)), EvexSecondHalf),
    row(Bug::EvexTwoTablesSelectorIgnored, "VPERMI2 and VPERMT2", Family::Evex(|form| form.reads_destination()), EvexSecondHalf),
    row(Bug::EvexLanesFromTheFirst, "EVEX in-lane shuffles", Family::Evex(|form| form.vector_bytes() > 16 && matches!(form.operation, EvexOperation::Integer { .. })), EvexSecondHalf),
    row(Bug::EvexShuffleBytesZeroBitIgnored, "VPSHUFB", Family::Evex(|form| is_integer(form, IntegerOperation::ShuffleBytes)), EvexSecondHalf),
    row(Bug::EvexUnpackSourcesSwapped, "VUNPCKH and VUNPCKL", Family::Evex(|form| is_integer(form, IntegerOperation::UnpackLow) || is_integer(form, IntegerOperation::UnpackHigh)), EvexSecondHalf),
    row(Bug::EvexAlignCountUnreduced, "VALIGND, VALIGNQ", Family::Evex(|form| is_integer(form, IntegerOperation::AlignElements)), EvexSecondHalf),
    row(Bug::EvexDotProductWraps, "VPDPBUSDS, VPDPWSSDS", Family::Evex(|form| matches!(form.operation, EvexOperation::Integer { operation: IntegerOperation::DotBytes { saturating: true } | IntegerOperation::DotWords { saturating: true }, .. })), EvexSecondHalf),
    row(Bug::EvexShiftDoubleHalvesSwapped, "VPSHLD, VPSHRD and their variable forms", Family::Evex(|form| matches!(form.operation, EvexOperation::Integer { operation: IntegerOperation::ShiftLeftDouble | IntegerOperation::ShiftRightDouble | IntegerOperation::ShiftLeftDoubleVariable | IntegerOperation::ShiftRightDoubleVariable, .. })), EvexSecondHalf),
    row(Bug::EvexConflictIncludesSelf, "VPCONFLICTD, VPCONFLICTQ", Family::Evex(|form| is_integer(form, IntegerOperation::Conflict)), EvexSecondHalf),
    row(Bug::EvexAesSkipsMixColumns, "VAESENC, VAESDEC", Family::Evex(|form| matches!(form.operation, EvexOperation::Integer { operation: IntegerOperation::Aes { last: false, .. }, .. })), EvexSecondHalf),
    row(Bug::EvexCarryLessIgnoresBitFour, "VPCLMULQDQ", Family::Evex(|form| is_integer(form, IntegerOperation::CarryLessMultiply)), EvexSecondHalf),
    row(Bug::EvexMaskDestinationMerges, "EVEX forms that write an opmask register", Family::Evex(EvexForm::writes_opmask), EvexSecondHalf),
    row(Bug::EvexFusedStickyIgnored, "EVEX fused multiply-add", Family::Evex(|form| matches!(form.operation, EvexOperation::Fused { .. })), EvexSecondHalf),
    row(Bug::EvexUnsignedNegativeWraps, "EVEX conversions to unsigned integers", Family::Evex(|form| matches!(form.operation, EvexOperation::Convert(Conversion { to: Number::Unsigned(_), .. }))), EvexSecondHalf),
    row(Bug::EvexBfloatKeepsDenormals, "VCVTNEPS2BF16, VCVTNE2PS2BF16", Family::Evex(|form| matches!(form.operation, EvexOperation::ToBfloat16 { .. })), EvexSecondHalf),
    row(Bug::HintsReadMemory, "prefetch hints, CLDEMOTE", Family::Forms(&[
        Instruction::Prefetchw0f0dr1, Instruction::Prefetchw0f0dr3, Instruction::PrefetchExclusiveMemmprefetch,
        Instruction::PrefetchReserved0f0dr4, Instruction::PrefetchReserved0f0dr5, Instruction::PrefetchReserved0f0dr6,
        Instruction::PrefetchReserved0f0dr7, Instruction::Prefetchwt1Memu8, Instruction::Prefetchit0Memu8,
        Instruction::Prefetchit1Memu8, Instruction::CldemoteMemu8,
    ]), EvexSecondHalf),
    row(Bug::RingThreeUndefinedFormsComplete, "SEAMCALL to VMFUNC", Family::Forms(&crate::RING_3_UNDEFINED_FORMS), EvexSecondHalf),
    row(Bug::EvexCompareSignalingBitIgnored, "VCMPPS to VCMPSD", Family::Evex(|form| matches!(form.operation, EvexOperation::Compare { .. })), EvexSecondHalf),
    row(Bug::EvexPermuteFaultsSuppressed, "EVEX permutes with memory", Family::Evex(|form| form.place != OperandPlace::Register && !form.suppresses_faults()), EvexSecondHalf),
];

/// Whether the EVEX form is an integer form of the operation.
fn is_integer(form: &EvexForm, operation: IntegerOperation) -> bool {
    matches!(form.operation, EvexOperation::Integer { operation: other, .. } if other == operation)
}

/// Whether the EVEX form computes floating-point elements from registers alone.
fn is_float_register_form(form: &EvexForm) -> bool {
    matches!(
        form.operation,
        EvexOperation::Float { .. }
            | EvexOperation::Fused { .. }
            | EvexOperation::Convert(_)
            | EvexOperation::Compare { .. }
            | EvexOperation::OrderedCompare { .. }
    )
        && form.place == OperandPlace::Register
        && form.code.op_code().can_suppress_all_exceptions()
}

#[cfg(feature = "planted-bugs")]
static ACTIVE: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// Turns the bug on, or every bug off for `None`; one bug at a time, for every thread.
#[cfg(feature = "planted-bugs")]
pub fn activate(bug: Option<Bug>) {
    let index = bug.map_or(0, |bug| {
        1 + PLANTED_BUGS.iter().position(|row| row.bug == bug).expect("every bug has a row")
    });
    ACTIVE.store(index, core::sync::atomic::Ordering::Relaxed);
}

/// The bug turned on, if any; always `None` without the `planted-bugs` feature.
pub fn active() -> Option<Bug> {
    #[cfg(feature = "planted-bugs")]
    {
        let index = ACTIVE.load(core::sync::atomic::Ordering::Relaxed);
        index.checked_sub(1).map(|index| PLANTED_BUGS[index].bug)
    }
    #[cfg(not(feature = "planted-bugs"))]
    {
        None
    }
}

/// Whether the bug is the one turned on.
pub fn is_active(bug: Bug) -> bool {
    active() == Some(bug)
}

/// The integer operation's outcome with the planted bug of its family.
#[expect(clippy::too_many_arguments, reason = "each is one input the operation reads")]
pub(crate) fn mutate_integer(
    operation: Operation,
    destination: u64,
    source: u64,
    length: usize,
    source_length: usize,
    [accumulator, data]: [u64; 2],
    count: u64,
    flags: Flags,
    undefined: UndefinedFlags,
    mut outcome: Outcome,
) -> Outcome {
    let Some(bug) = active() else {
        return outcome;
    };
    let mask = to_mask(length);
    let bits = length as u32 * 8;
    let sign = 1u64 << (bits - 1);
    let (d, s) = (destination & mask, source & mask);
    let carry = flags.contains(Flags::CARRY);
    let result = outcome.result.unwrap_or(0) & mask;
    match (bug, operation) {
        (Bug::AddAuxiliaryFromSumBitFour, Operation::Add | Operation::AddWithCarry) => {
            outcome.flags.set(Flags::AUXILIARY_CARRY, result & 0x10 != 0);
        }
        (Bug::AddAuxiliaryFromBitFive, Operation::Add) => {
            outcome.flags.set(Flags::AUXILIARY_CARRY, (d ^ s ^ result) & 0x20 != 0);
        }
        (Bug::AddOverflowIgnoresSourceSign, Operation::Add) => outcome.flags.set(Flags::OVERFLOW, (d ^ result) & sign != 0),
        (Bug::AddWithCarryCarryIgnoresCarryIn, Operation::AddWithCarry) => {
            outcome.flags.set(Flags::CARRY, u128::from(d) + u128::from(s) > u128::from(mask));
        }
        (Bug::AddWithCarryOverflowIgnoresCarryIn, Operation::AddWithCarry) => {
            let plain = d.wrapping_add(s) & mask;
            outcome.flags.set(Flags::OVERFLOW, (d ^ plain) & (s ^ plain) & sign != 0);
        }
        (Bug::SubtractWithBorrowAddsBorrow, Operation::SubtractWithBorrow) => {
            let wrong = d.wrapping_sub(s).wrapping_add(u64::from(carry)) & mask;
            outcome.result = Some(wrong);
            outcome.flags = with_status(outcome.flags, to_result_flags(wrong, length), Flags::ZERO | Flags::SIGN | Flags::PARITY);
        }
        (Bug::SubtractWithBorrowCarryIgnoresBorrow, Operation::SubtractWithBorrow) => outcome.flags.set(Flags::CARRY, d < s),
        (Bug::SubtractOverflowFromSourceSign, Operation::Subtract) => {
            outcome.flags.set(Flags::OVERFLOW, (d ^ s) & (s ^ result) & sign != 0);
        }
        (Bug::CompareWritesDifference, Operation::Compare) => outcome.result = Some(d.wrapping_sub(s) & mask),
        (Bug::NegateOverflowNeverSet, Operation::Negate) => outcome.flags.remove(Flags::OVERFLOW),
        (Bug::NegateAuxiliaryCleared, Operation::Negate) => outcome.flags.remove(Flags::AUXILIARY_CARRY),
        (Bug::IncrementSetsCarry, Operation::Increment) => outcome.flags.set(Flags::CARRY, d == mask),
        (Bug::IncrementOverflowOnlyAtSixtyFourBits, Operation::Increment) => {
            let overflow = length == 8 && outcome.flags.contains(Flags::OVERFLOW);
            outcome.flags.set(Flags::OVERFLOW, overflow);
        }
        (Bug::DecrementAuxiliaryFromBitThree, Operation::Decrement) => {
            outcome.flags.set(Flags::AUXILIARY_CARRY, (d ^ 1 ^ result) & 0x08 != 0);
        }
        (Bug::AndCarryKept, Operation::And) => outcome.flags.set(Flags::CARRY, carry),
        (Bug::OrParityOverWholeOperands, Operation::Or) => {
            outcome.flags.set(Flags::PARITY, (destination | source).count_ones().is_multiple_of(2));
        }
        (Bug::XorOverflowKept, Operation::Xor) => outcome.flags.set(Flags::OVERFLOW, flags.contains(Flags::OVERFLOW)),
        (Bug::TestSignFromBitSixtyThree, Operation::Test) => outcome.flags.set(Flags::SIGN, (destination & source) >> 63 != 0),
        (Bug::NotWritesResultFlags, Operation::Not) => {
            outcome.flags = with_status(flags, to_result_flags(result, length), Flags::STATUS);
        }
        (Bug::MoveIfFalseKeepsUpperHalf, Operation::MoveIf(condition)) if !condition.holds(flags) => outcome.result = None,
        (Bug::AddParityFromLowSixteenBits, Operation::Add) => {
            outcome.flags.set(Flags::PARITY, ((result & 0xFFFF) as u16).count_ones().is_multiple_of(2));
        }
        (Bug::AddWithCarryAuxiliaryIgnoresCarryIn, Operation::AddWithCarry) => {
            outcome.flags.set(Flags::AUXILIARY_CARRY, (d ^ s ^ (d.wrapping_add(s) & mask)) & 0x10 != 0);
        }
        (Bug::SubtractWithBorrowOverflowIgnoresBorrow, Operation::SubtractWithBorrow) => {
            let plain = d.wrapping_sub(s) & mask;
            outcome.flags.set(Flags::OVERFLOW, (d ^ s) & (d ^ plain) & sign != 0);
        }
        (Bug::AndSignFromBitSeven, Operation::And) => outcome.flags.set(Flags::SIGN, result & 0x80 != 0),
        (Bug::CompareCarryWhenEqual, Operation::Compare) => outcome.flags.set(Flags::CARRY, d <= s),
        (Bug::TestCarryKept, Operation::Test) => outcome.flags.set(Flags::CARRY, carry),
        (Bug::IncrementAuxiliaryFromBitFour, Operation::Increment) => {
            outcome.flags.set(Flags::AUXILIARY_CARRY, (d ^ 1 ^ result) & 0x20 != 0);
        }
        (Bug::DecrementOverflowFromBitSixtyThree, Operation::Decrement) => {
            outcome.flags.set(Flags::OVERFLOW, (d ^ 1) & (d ^ result) & (1 << 63) != 0);
        }
        (Bug::NegateCarryWhenZero, Operation::Negate) => outcome.flags.set(Flags::CARRY, d == 0),
        (Bug::ShiftLeftWordCountFourBits, Operation::ShiftLeft) if length == 2 => {
            outcome = operation.shift(destination, source & 0xF, length, flags, undefined);
        }
        (Bug::ShiftRightByOneOverflowCleared, Operation::ShiftRightLogical) if source & 0x1F == 1 => {
            outcome.flags.remove(Flags::OVERFLOW);
        }
        (Bug::ShiftRightArithmeticWordAsLogical, Operation::ShiftRightArithmetic) if length == 2 => {
            outcome = Operation::ShiftRightLogical.shift(destination, source, length, flags, undefined);
        }
        (Bug::RotateLeftDoublewordCountSixBits, Operation::RotateLeft) if length == 4 && (source & 0x3F) >= 32 => {
            let rotated = u64::from((d as u32).rotate_left((source & 0x3F) as u32 % 32));
            outcome.result = Some(rotated);
            outcome.flags.set(Flags::CARRY, rotated & 1 != 0);
            outcome.flags.set(Flags::OVERFLOW, (rotated >> 31 != 0) != (rotated & 1 != 0));
        }
        (Bug::RotateRightCarryFromBitZero, Operation::RotateRight)
            if source & if length == 8 { 0x3F } else { 0x1F } != 0 =>
        {
            outcome.flags.set(Flags::CARRY, result & 1 != 0);
        }
        (Bug::RotateLeftThroughCarryWordModuloSixteen, Operation::RotateLeftThroughCarry) if length == 2 => {
            outcome = operation.shift(destination, (source & 0x1F) % 16, length, flags, undefined);
        }
        (Bug::RotateRightThroughCarryByteModuloEight, Operation::RotateRightThroughCarry) if length == 1 => {
            outcome = operation.shift(destination, (source & 0x1F) % 8, length, flags, undefined);
        }
        (Bug::MoveIfNotLessOrEqualIgnoresZero, Operation::MoveIf(Condition::NotLessOrEqual)) => {
            let holds = flags.contains(Flags::SIGN) == flags.contains(Flags::OVERFLOW);
            outcome.result = Some(if holds { source } else { destination } & mask);
        }
        (Bug::SetIfBelowOrEqualIgnoresZero, Operation::SetIf(Condition::BelowOrEqual)) => outcome.result = Some(u64::from(carry)),
        (Bug::MultiplySignedOverflowFromUnsignedProduct, Operation::MultiplySigned) => {
            let wide = u128::from(d) * u128::from(s);
            outcome.flags.set(Flags::CARRY, wide > u128::from(mask));
            outcome.flags.set(Flags::OVERFLOW, wide > u128::from(mask));
        }
        (Bug::ZeroExtendWordReadsByte, Operation::ZeroExtend) if source_length == 2 => outcome.result = Some(source & 0xFF),
        (Bug::SignExtendByteDropsSign, Operation::SignExtend) if source_length == 1 => outcome.result = Some(source & 0xFF),
        (Bug::SignExtendDoublewordZeroExtends, Operation::SignExtend) if source_length == 4 => {
            outcome.result = Some(source & 0xFFFF_FFFF);
        }
        (Bug::ExchangeAddSwapped, Operation::ExchangeAdd) => core::mem::swap(&mut outcome.result, &mut outcome.second),
        (Bug::MultiplyWideByteHighIntoDl, Operation::MultiplyWide { signed: false }) if length == 1 => {
            if let Some((_, high)) = outcome.implicit[1] {
                outcome.implicit[1] = Some((Register::DL, high));
            }
        }
        (Bug::MultiplyWideSignedOverflowFromNonzeroHigh, Operation::MultiplyWide { signed: true }) => {
            if let Some((_, high)) = outcome.implicit[1] {
                outcome.flags.set(Flags::CARRY, high & mask != 0);
                outcome.flags.set(Flags::OVERFLOW, high & mask != 0);
            }
        }
        (Bug::DivideTooWideTruncated, Operation::DivideWide { signed: false }) if outcome.exception.is_some() && d != 0 => {
            let (low, high) = if length == 1 { (accumulator & 0xFF, (accumulator >> 8) & 0xFF) } else { (accumulator & mask, data & mask) };
            let dividend = (u128::from(high) << bits) | u128::from(low);
            outcome.exception = None;
            outcome.implicit = [
                Some((to_accumulator(length), (dividend / u128::from(d)) as u64 & mask)),
                Some((to_data(length), (dividend % u128::from(d)) as u64 & mask)),
            ];
        }
        (Bug::DivideSignedRemainderTakesDivisorSign, Operation::DivideWide { signed: true }) if outcome.exception.is_none() => {
            if let Some((register, remainder)) = outcome.implicit[1] {
                let mut value = sign_extend(remainder, length) as i64;
                let divisor = sign_extend(destination, length) as i64;
                if value != 0 && (value < 0) != (divisor < 0) {
                    value = value.wrapping_add(divisor);
                }
                outcome.implicit[1] = Some((register, value as u64 & mask));
            }
        }
        (Bug::SignExtendAccumulatorDoublewordZeroExtends, Operation::SignExtendAccumulator) if length == 8 => {
            outcome.implicit[0] = Some((to_accumulator(8), accumulator & 0xFFFF_FFFF));
        }
        (Bug::SignExtendAccumulatorByteFromBitFifteen, Operation::SignExtendAccumulator) if length == 2 => {
            outcome.implicit[0] = Some((to_accumulator(2), accumulator & 0xFFFF));
        }
        (Bug::SignIntoDataQuadwordFromBitThirtyOne, Operation::SignIntoData) if length == 8 => {
            outcome.implicit[0] = Some((to_data(8), if accumulator & (1 << 31) != 0 { u64::MAX } else { 0 }));
        }
        (Bug::LoadStatusIntoHighBitOneClear, Operation::LoadStatusIntoHigh) => {
            if let Some((register, high)) = outcome.implicit[0] {
                outcome.implicit[0] = Some((register, high & !2));
            }
        }
        (Bug::StoreHighIntoStatusClearsOverflow, Operation::StoreHighIntoStatus) => outcome.flags.remove(Flags::OVERFLOW),
        (Bug::ComplementCarrySets, Operation::ComplementCarry) => outcome.flags.insert(Flags::CARRY),
        (Bug::ShiftLeftDoubleDoublewordCountFourBits, Operation::ShiftLeftDouble) if length == 4 => {
            outcome = operation.shift_double(destination, source, count & 0xF, length, flags, undefined);
        }
        (Bug::ShiftRightDoubleByOneOverflowCleared, Operation::ShiftRightDouble) if count & 0x1F == 1 => {
            outcome.flags.remove(Flags::OVERFLOW);
        }
        (Bug::BitComplementQuadwordOffsetFiveBits, Operation::BitTest(BitChange::Complement)) if length == 8 => {
            outcome = crate::integer::test_bit(BitChange::Complement, destination, source & 0x1F, length, flags, undefined);
        }
        (Bug::BitSetCarryAfterSet, Operation::BitTest(BitChange::Set)) => outcome.flags.insert(Flags::CARRY),
        (Bug::CompareExchangeMismatchKeepsUpperHalf, Operation::CompareExchange) if length == 4 && d != accumulator & mask => {
            outcome.implicit[0] = Some((Register::RAX, (accumulator & !0xFFFF_FFFF) | d));
        }
        (Bug::CompareExchangeMismatchWritesBack, Operation::CompareExchange) if length == 4 && d != accumulator & mask => {
            outcome.result = Some(d);
        }
        (Bug::CompareExchangeByteFlagsReversed, Operation::CompareExchange) if length == 1 => {
            let (_, status) = subtract_with_borrow(d, accumulator & mask, false, length);
            outcome.flags = with_status(flags, status, Flags::STATUS);
        }
        (Bug::BitScanReverseZeroZeroes, Operation::BitScanReverse) if s == 0 => outcome.result = Some(0),
        (Bug::BitScanForwardParityFromSource, Operation::BitScanForward) if s != 0 => {
            outcome.flags.set(Flags::PARITY, (s as u8).count_ones().is_multiple_of(2));
        }
        (Bug::PopulationCountKeepsCarry, Operation::PopulationCount) => outcome.flags.set(Flags::CARRY, carry),
        (Bug::ByteSwapQuadwordSwapsThirtyTwo, Operation::ByteSwap) if length == 8 => {
            outcome.result = Some(u64::from((d as u32).swap_bytes()));
        }
        // Fifth check.
        (Bug::SubtractAuxiliaryFromResultBitFour, Operation::Subtract) => {
            outcome.flags.set(Flags::AUXILIARY_CARRY, result & 0x10 != 0);
        }
        (Bug::CompareOverflowFromDestinationSign, Operation::Compare) => {
            let difference = d.wrapping_sub(s) & mask;
            outcome.flags.set(Flags::OVERFLOW, (d ^ difference) & sign != 0);
        }
        (Bug::AddWithCarryZeroFromUntruncatedSum, Operation::AddWithCarry) => {
            outcome.flags.set(Flags::ZERO, u128::from(d) + u128::from(s) + u128::from(carry) == 0);
        }
        (Bug::SubtractWithBorrowSignFromBitSixtyThree, Operation::SubtractWithBorrow) if length == 4 => {
            outcome.flags.set(Flags::SIGN, d.wrapping_sub(s).wrapping_sub(u64::from(carry)) >> 63 != 0);
        }
        (Bug::OrCarryKept, Operation::Or) => outcome.flags.set(Flags::CARRY, carry),
        (Bug::NegateSignFromBitSixtyThree, Operation::Negate) => outcome.flags.set(Flags::SIGN, d.wrapping_neg() >> 63 != 0),
        (Bug::IncrementClearsCarry, Operation::Increment) => outcome.flags.remove(Flags::CARRY),
        (Bug::DecrementSetsCarryAtZero, Operation::Decrement) => outcome.flags.set(Flags::CARRY, d == 0),
        (Bug::ShiftRightDoublewordCountSixBits, Operation::ShiftRightLogical) if length == 4 && source & 0x3F >= 32 => {
            outcome.result = Some(0);
            outcome.flags = with_status(outcome.flags, to_result_flags(0, length), Flags::ZERO | Flags::SIGN | Flags::PARITY);
            outcome.flags.set(Flags::CARRY, source & 0x3F == 32 && d >> 31 != 0);
        }
        (Bug::ShiftRightArithmeticQuadwordCountFiveBits, Operation::ShiftRightArithmetic)
        | (Bug::RotateRightThroughCarryQuadwordCountFiveBits, Operation::RotateRightThroughCarry)
            if length == 8 =>
        {
            outcome = operation.shift(destination, source & 0x1F, length, flags, undefined);
        }
        (Bug::RotateLeftByteCountThreeBits, Operation::RotateLeft) if length == 1 => {
            outcome = operation.shift(destination, source & 0x7, length, flags, undefined);
        }
        (Bug::ShiftLeftOneOverflowFromCarryIn, Operation::ShiftLeft)
            if source & if length == 8 { 0x3F } else { 0x1F } == 1 =>
        {
            outcome.flags.set(Flags::OVERFLOW, (result & sign != 0) != carry);
        }
        (Bug::SignExtendWordZeroExtends, Operation::SignExtend) if source_length == 2 => outcome.result = Some(source & 0xFFFF),
        (Bug::ZeroExtendByteSignExtends, Operation::ZeroExtend) if source_length == 1 => {
            outcome.result = Some(sign_extend(source & 0xFF, 1) & mask);
        }
        (Bug::MultiplySignedDoublewordNeverOverflows, Operation::MultiplySigned) if length == 4 => {
            outcome.flags.remove(Flags::CARRY | Flags::OVERFLOW);
        }
        (Bug::MultiplyWideQuadwordHalvesSwapped, Operation::MultiplyWide { signed: false }) if length == 8 => {
            if let [Some((low_register, low)), Some((high_register, high))] = outcome.implicit {
                outcome.implicit = [Some((low_register, high)), Some((high_register, low))];
            }
        }
        (Bug::DivideSignedMostNegativeQuotientFaults, Operation::DivideWide { signed: true }) if outcome.exception.is_none() => {
            if outcome.implicit[0].is_some_and(|(_, quotient)| quotient & mask == sign) {
                outcome.exception = Some(crate::Exception::DivideError);
            }
        }
        (Bug::ExchangeAddOverflowIgnoresSourceSign, Operation::ExchangeAdd) => {
            let sum = d.wrapping_add(s) & mask;
            outcome.flags.set(Flags::OVERFLOW, (d ^ sum) & sign != 0);
        }
        (Bug::MoveIfBelowOrEqualIgnoresCarry, Operation::MoveIf(Condition::BelowOrEqual)) => {
            outcome.result = Some(if flags.contains(Flags::ZERO) { source } else { destination } & mask);
        }
        (Bug::SetIfGreaterIgnoresZero, Operation::SetIf(Condition::NotLessOrEqual)) => {
            outcome.result = Some(u64::from(flags.contains(Flags::SIGN) == flags.contains(Flags::OVERFLOW)));
        }
        (Bug::BitTestWordOffsetThreeBits, Operation::BitTest(BitChange::Keep)) if length == 2 => {
            outcome = crate::integer::test_bit(BitChange::Keep, destination, source & 0x7, length, flags, undefined);
        }
        (Bug::ShiftLeftDoubleQuadwordCountFiveBits, Operation::ShiftLeftDouble) if length == 8 => {
            outcome = operation.shift_double(destination, source, count & 0x1F, length, flags, undefined);
        }
        (Bug::BitScanReverseFromTop, Operation::BitScanReverse) if s != 0 => {
            outcome.result = Some(u64::from(bits - 1 - (63 - s.leading_zeros())));
        }
        (Bug::PopulationCountLowHalfOnly, Operation::PopulationCount) if length == 8 => {
            outcome.result = Some(u64::from((s as u32).count_ones()));
            outcome.flags.set(Flags::ZERO, s as u32 == 0);
        }
        _ => {}
    }
    outcome
}

/// The lane operation's result with the planted bug of its family.
pub(crate) fn mutate_lanes(operation: LaneOperation, destination: u128, source: u128, immediate: u8, result: u128) -> u128 {
    use Lane::{Byte, Doubleword, Quadword, Word};
    use Signedness::{Signed, Unsigned};
    let Some(bug) = active() else {
        return result;
    };
    let lane = operation.to_lane();
    let each = |combine: &dyn Fn(u64, u64) -> u64| {
        join(lane, split(lane, destination).iter().zip(split(lane, source)).map(|(&first, second)| combine(first, second)))
    };
    let mask = to_mask(lane.to_bits() as usize / 8);
    match (bug, operation) {
        (Bug::AddDoublewordsCarryAcrossLanes, LaneOperation::Add(Doubleword))
        | (Bug::AddQuadwordsCarryAcross, LaneOperation::Add(Quadword)) => destination.wrapping_add(source),
        (Bug::AddSaturatingWordsUnsigned, LaneOperation::AddSaturating(Word, Signed)) => {
            LaneOperation::AddSaturating(Word, Unsigned).compute(destination, source, immediate)
        }
        (Bug::AddSaturatingBytesSigned, LaneOperation::AddSaturating(Byte, Unsigned)) => {
            LaneOperation::AddSaturating(Byte, Signed).compute(destination, source, immediate)
        }
        (Bug::SubtractSaturatingBytesWraps, LaneOperation::SubtractSaturating(Byte, Unsigned)) => {
            LaneOperation::Subtract(Byte).compute(destination, source, immediate)
        }
        (Bug::AverageBytesNotRounded, LaneOperation::Average(Byte)) => each(&|first, second| (first + second) >> 1),
        (Bug::CompareGreaterWordsUnsigned, LaneOperation::CompareGreater(Word)) => {
            each(&|first, second| if first > second { mask } else { 0 })
        }
        (Bug::MultiplyHighWordsUnsigned, LaneOperation::MultiplyHighWords(Signed)) => {
            LaneOperation::MultiplyHighWords(Unsigned).compute(destination, source, immediate)
        }
        (Bug::ShiftRightArithmeticWordsWideCountZero, LaneOperation::ShiftRightArithmetic(Word)) if source as u64 >= 16 => 0,
        (Bug::ShiftLeftQuadwordsCountSixBits, LaneOperation::ShiftLeft(Quadword)) => {
            operation.compute(destination, u128::from(source as u64 & 63), immediate)
        }
        (Bug::PackUnsignedReadsUnsigned, LaneOperation::Pack(Word, Unsigned)) => {
            let lanes = split(Word, destination).into_iter().chain(split(Word, source));
            join(Byte, lanes.map(|value| saturate(Byte, Unsigned, i128::from(value))))
        }
        (Bug::MultiplyAddWordsSaturates, LaneOperation::MultiplyAddWords) => {
            let (first, second) = (split(Word, destination), split(Word, source));
            let products: alloc::vec::Vec<i64> =
                first.iter().zip(&second).map(|(&a, &b)| to_signed(a, 16) * to_signed(b, 16)).collect();
            join(Doubleword, products.chunks(2).map(|pair| saturate(Doubleword, Signed, i128::from(pair[0] + pair[1]))))
        }
        (Bug::MaximumSignedBytesUnsigned, LaneOperation::Maximum(Byte, Signed)) => {
            LaneOperation::Maximum(Byte, Unsigned).compute(destination, source, immediate)
        }
        (Bug::CompareGreaterQuadwordsUnsigned, LaneOperation::CompareGreater(Quadword)) => {
            each(&|first, second| if first > second { mask } else { 0 })
        }
        (Bug::UnpackHighDoublewordsTakesLow, LaneOperation::UnpackHigh(Doubleword)) => {
            LaneOperation::UnpackLow(Doubleword).compute(destination, source, immediate)
        }
        (Bug::PackUnsignedDoublewordsReadsUnsigned, LaneOperation::Pack(Doubleword, Unsigned)) => {
            let lanes = split(Doubleword, destination).into_iter().chain(split(Doubleword, source));
            join(Word, lanes.map(|value| saturate(Word, Unsigned, i128::from(value))))
        }
        (Bug::ShuffleBytesIgnoresZeroingBit, LaneOperation::ShuffleBytes) => {
            let bytes = split(Byte, destination);
            each(&|_, index| bytes[(index & 0xF) as usize])
        }
        (Bug::SignWordsKeepsForZero, LaneOperation::Sign(Word)) => {
            each(&|first, second| if to_signed(second, 16) < 0 { first.wrapping_neg() & mask } else { first })
        }
        (Bug::HorizontalSubtractWordsReversed, LaneOperation::HorizontalSubtract(Word)) => {
            let lanes: alloc::vec::Vec<u64> = split(Word, destination).into_iter().chain(split(Word, source)).collect();
            join(Word, lanes.chunks(2).map(|pair| pair[1].wrapping_sub(pair[0]) & mask))
        }
        (Bug::MultiplyAddBytesBothSigned, LaneOperation::MultiplyAddBytes) => {
            let (first, second) = (split(Byte, destination), split(Byte, source));
            let products: alloc::vec::Vec<i128> =
                first.iter().zip(&second).map(|(&a, &b)| i128::from(to_signed(a, 8) * to_signed(b, 8))).collect();
            join(Word, products.chunks(2).map(|pair| saturate(Word, Signed, pair[0] + pair[1])))
        }
        (Bug::MultiplyHighRoundScaleNotRounded, LaneOperation::MultiplyHighRoundScale) => {
            each(&|first, second| ((to_signed(first, 16) * to_signed(second, 16)) >> 15) as u64 & mask)
        }
        (Bug::AbsoluteBytesSaturates, LaneOperation::Absolute(Byte)) => {
            each(&|_, second| (to_signed(second, 8).unsigned_abs()).min(0x7F))
        }
        (Bug::MultiplyDoublewordsSignedAsUnsigned, LaneOperation::MultiplyDoublewords(Signed)) => {
            LaneOperation::MultiplyDoublewords(Unsigned).compute(destination, source, immediate)
        }
        (Bug::HorizontalAddSaturatingWraps, LaneOperation::HorizontalAddSaturating) => {
            LaneOperation::HorizontalAdd(Word).compute(destination, source, immediate)
        }
        (Bug::ShuffleDoublewordsFieldsReversed, LaneOperation::ShuffleDoublewords) => {
            let lanes = split(Doubleword, source);
            join(Doubleword, (0..4).map(|index| lanes[usize::from((immediate >> (2 * (3 - index))) & 3)]))
        }
        (Bug::AlignRightCountFourBits, LaneOperation::AlignRight) => operation.compute(destination, source, immediate & 0xF),
        (Bug::BlendSinglesBitsReversed, LaneOperation::Blend(Doubleword)) => {
            let reversed = (0..4).fold(0, |all, bit| all | (((immediate >> bit) & 1) << (3 - bit)));
            operation.compute(destination, source, reversed)
        }
        (Bug::ShiftBytesRightKeepsTopByte, LaneOperation::ShiftBytesRight) if source >= 16 => destination >> 120,
        (Bug::DuplicateOddSinglesTakesEven, LaneOperation::DuplicateOddSingles) => {
            LaneOperation::DuplicateEvenSingles.compute(destination, source, immediate)
        }
        (Bug::MoveScalarFromMemoryKeepsUpper, LaneOperation::MoveLowZeroing(Doubleword)) => {
            LaneOperation::MoveLow(Doubleword).compute(destination, source, immediate)
        }
        (Bug::ExtendBytesSignedAsUnsigned, LaneOperation::Extend { from: Byte, to: Word, signedness: Signed }) => {
            LaneOperation::Extend { from: Byte, to: Word, signedness: Unsigned }.compute(destination, source, immediate)
        }
        (Bug::MoveHighToLowTakesLow, LaneOperation::MoveHighToLow) => (destination & !u128::from(u64::MAX)) | (source & u128::from(u64::MAX)),
        (Bug::TestCarryFromAnd, LaneOperation::Test) => (result & !1) | u128::from(destination & source == 0),
        (Bug::MinimumWordPositionLastIndex, LaneOperation::MinimumWordPosition) => {
            let words = split(Word, source);
            let least = words.iter().min().copied().unwrap_or(0);
            let index = words.iter().rposition(|&word| word == least).unwrap_or(0);
            u128::from(least) | ((index as u128) << 16)
        }
        (Bug::InsertSingleIgnoresZeroMask, LaneOperation::InsertSingle { .. }) => {
            operation.compute(destination, source, immediate & 0xF0)
        }
        (Bug::UnpackHighQuadwordsTakesLow, LaneOperation::UnpackHigh(Quadword)) => {
            LaneOperation::UnpackLow(Quadword).compute(destination, source, immediate)
        }
        (Bug::UnpackLowBytesSourceFirst, LaneOperation::UnpackLow(Byte)) => {
            LaneOperation::UnpackLow(Byte).compute(source, destination, immediate)
        }
        (Bug::SumAbsoluteDifferencesWhole, LaneOperation::SumAbsoluteDifferences) => {
            let sum: u64 = split(Byte, destination).iter().zip(split(Byte, source)).map(|(&a, b)| a.abs_diff(b)).sum();
            u128::from(sum)
        }
        // Fifth check.
        (Bug::SubtractSaturatingWordsUnsigned, LaneOperation::SubtractSaturating(Word, Signed)) => {
            LaneOperation::SubtractSaturating(Word, Unsigned).compute(destination, source, immediate)
        }
        (Bug::MultiplyLowWordsGivesHigh, LaneOperation::MultiplyLow(Word)) => {
            LaneOperation::MultiplyHighWords(Signed).compute(destination, source, immediate)
        }
        (Bug::ShiftRightLogicalWordsCountFourBits, LaneOperation::ShiftRightLogical(Word)) => {
            operation.compute(destination, u128::from(source as u64 & 0xF), immediate)
        }
        (Bug::ShiftRightArithmeticDoublewordsWideZero, LaneOperation::ShiftRightArithmetic(Doubleword)) if source as u64 >= 32 => 0,
        (Bug::PackSignedDoublewordsSaturatesUnsigned, LaneOperation::Pack(Doubleword, Signed)) => {
            LaneOperation::Pack(Doubleword, Unsigned).compute(destination, source, immediate)
        }
        (Bug::MinimumWordsUnsignedAsSigned, LaneOperation::Minimum(Word, Unsigned)) => {
            LaneOperation::Minimum(Word, Signed).compute(destination, source, immediate)
        }
        (Bug::UnpackLowWordsTakesHigh, LaneOperation::UnpackLow(Word)) => {
            LaneOperation::UnpackHigh(Word).compute(destination, source, immediate)
        }
        (Bug::ShuffleLowWordsShufflesHigh, LaneOperation::ShuffleLowWords) => {
            let low = u128::from(u64::MAX);
            (result & low) | (LaneOperation::ShuffleHighWords.compute(destination, source, immediate) & !low)
        }
        (Bug::MultiplyUnsignedDoublewordsOddLanes, LaneOperation::MultiplyDoublewords(Unsigned)) => {
            let even = 0x0000_0000_FFFF_FFFF_0000_0000_FFFF_FFFFu128;
            operation.compute((destination >> 32) & even, (source >> 32) & even, immediate)
        }
        (Bug::ShiftBytesLeftCountFourBits, LaneOperation::ShiftBytesLeft) => operation.compute(destination, source & 0xF, immediate),
        (Bug::AbsoluteDoublewordsSaturates, LaneOperation::Absolute(Doubleword)) => {
            each(&|_, second| to_signed(second, 32).unsigned_abs().min(0x7FFF_FFFF))
        }
        (Bug::HorizontalAddDoublewordsSourceLow, LaneOperation::HorizontalAdd(Doubleword)) => result.rotate_left(64),
        (Bug::ExtendBytesToDoublewordsZeroAsSigned, LaneOperation::Extend { from: Byte, to: Doubleword, signedness: Unsigned }) => {
            LaneOperation::Extend { from: Byte, to: Doubleword, signedness: Signed }.compute(destination, source, immediate)
        }
        (Bug::MinimumDoublewordsSignedAsUnsigned, LaneOperation::Minimum(Doubleword, Signed)) => {
            LaneOperation::Minimum(Doubleword, Unsigned).compute(destination, source, immediate)
        }
        (Bug::BlendWordsBitsReversed, LaneOperation::Blend(Word)) => operation.compute(destination, source, immediate.reverse_bits()),
        (Bug::CompareGreaterBytesUnsigned, LaneOperation::CompareGreater(Byte)) => {
            each(&|first, second| if first > second { mask } else { 0 })
        }
        (Bug::ShuffleSinglesOperandsSwapped, LaneOperation::ShuffleSingles) => operation.compute(source, destination, immediate),
        (Bug::MoveLowToHighTakesHigh, LaneOperation::MoveLowToHigh) => {
            let low = u128::from(u64::MAX);
            (destination & low) | (source & !low)
        }
        (Bug::AndNotReversed, LaneOperation::AndNot) => destination & !source,
        _ => result,
    }
}
