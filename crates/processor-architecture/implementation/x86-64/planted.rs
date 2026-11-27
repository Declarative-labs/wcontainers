//! Named mutations of the simulation that the generated states must catch, each with its family of forms and the
//! pass or check that planted it, which only the `planted-bugs` feature can turn on.

use iced_x86::Register;

use crate::behavior::UndefinedFlags;
use crate::evex::{Conversion, EvexForm, EvexOperation, IntegerOperation, Number, SpecialOperation, find_evex_form};
use crate::float::{FloatForm, FloatOperation, Precision, find_float_form};
use crate::integer::{
    BitChange, Condition, OperandPlace, Operation, Outcome, find_integer_form, sign_extend, subtract_with_borrow,
    to_accumulator, to_data, to_mask, to_result_flags, with_status,
};
use crate::vector::{Lane, LaneOperation, Signedness, VectorEncoding, find_vector_form, join, saturate, split, to_signed};
use crate::sse::{Combination, LaneSelect, SseOperation};
use crate::float::Arrangement;
use crate::avx::{AvxForm, AvxOperation, Wide, find_avx_form};
use crate::avx512_integer::{EvexIntegerForm, EvexIntegerOperation, EvexIntegerPlace, ComparePredicate, Zmm, find_evex_integer_form, from_lanes, to_lanes};
use crate::opmask::{OpmaskOperation, find_opmask_form};
use crate::optional::{OptionalForm, OptionalOperation};
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
    /// The pass that models the rest of SSE to SSE4.2, AES, PCLMULQDQ, SHA and GFNI.
    SseFamily,
    /// The AVX family: AVX, AVX2, FMA, F16C and the VEX forms of AES, PCLMULQDQ and GFNI.
    Avx,
    /// The family of the legacy integer forms left, x87, MMX and the general-purpose extensions.
    IntegerX87Mmx,
    /// The second half of AVX-512's forms.
    EvexSecondHalf,
    /// The opmask forms and the foundation set's EVEX integer forms.
    Avx512Integer,
    /// The sync onto the owner's expectations, for RDRAND and RDSEED.
    Expectations,
    /// The family of the forms every x86-64 chip has that were left, and of the forms the chip source refused.
    Mandatory,
    /// The VEX family's second pass: ZMM's bits above 255, AVX-VNNI and the XSAVE family.
    VexSecondPass,
    /// The rest of AVX-512's first half: the byte and word forms, packs, unpacks, broadcasts, moves and the rest.
    EvexFirstHalf,
    /// The second EVEX pass: AVX512-FP16 and the rest of AVX-512's second half, and the small sets left.
    EvexRest,
    /// The optional forms the chip runs that had no model, and the generator's holes the check found.
    OptionalRemaining,
    /// The family of XCR0, XINUSE and the XSAVE forms, and of UMWAIT and TPAUSE.
    StateAndXsave,
    /// The family of the x87 transcendental forms, JCXZ and the forms the chip source refused.
    X87AndRefused,
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
    /// The forms of [`crate::sse::SSE_FORMS`] whose operation the test accepts.
    Sse(fn(SseOperation) -> bool),
    /// The EVEX forms of [`crate::evex`] the test accepts.
    Evex(fn(&EvexForm) -> bool),
    Every,
    /// The forms of [`crate::avx::AVX_FORMS`] the test accepts.
    Avx(fn(&AvxForm) -> bool),
    /// The opmask forms whose operation the test accepts.
    Opmask(fn(OpmaskOperation) -> bool),
    /// The EVEX integer forms the test accepts.
    EvexInteger(fn(&EvexIntegerForm) -> bool),
    /// The string forms the test accepts.
    String(fn(&crate::string::StringForm) -> bool),
    /// The forms of [`crate::optional::OPTIONAL_FORMS`] the test accepts.
    OptionalForms(fn(&OptionalForm) -> bool),
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
                    || crate::sse::find_sse_form(instruction).is_some_and(|form| form.place != OperandPlace::Register)
            }
            Family::Branch => crate::branch::find_branch_form(instruction).is_some(),
            Family::Forms(forms) => forms.contains(&instruction),
            Family::VexVector => find_vector_form(instruction).is_some_and(|form| form.encoding != VectorEncoding::Legacy),
            Family::Optional => crate::is_optional(instruction),
            Family::Sse(test) => crate::sse::find_sse_form(instruction).is_some_and(|form| test(form.operation)),
            Family::Evex(test) => find_evex_form(instruction).is_some_and(|form| test(&form)),
            Family::Every => true,
            Family::Avx(test) => find_avx_form(instruction).is_some_and(test),
            Family::Opmask(test) => find_opmask_form(instruction).is_some_and(|form| test(form.operation)),
            Family::EvexInteger(test) => find_evex_integer_form(instruction).is_some_and(test),
            Family::String(test) => crate::string::find_string_form(instruction).is_some_and(test),
            Family::OptionalForms(test) => crate::optional::find_optional_form(instruction).is_some_and(test),
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
    // The SSE family's pass.
    /// PEXTRB to PINSRQ and EXTRACTPS take lane 0 for an immediate beyond the lanes.
    LaneSelectorUnmasked,
    /// MOVHPS and MOVHPD store the low quadword.
    HighQuadwordStoreReadsLow,
    /// A lane moved to a 32-bit register keeps the upper half.
    ExtractKeepsUpperHalf,
    /// MOVD, MOVQ, LDDQU and MOVNTDQA into an XMM register keep the lanes above.
    InsertZeroingKeepsUpperLanes,
    /// MOVMSKPS, MOVMSKPD and PMOVMSKB take each lane's lowest bit.
    MoveMaskFromLowBits,
    /// MOVAPS to memory does not fault when misaligned.
    AlignedStoreMisalignedNoFault,
    /// MOVNTDQA does not fault when misaligned.
    NonTemporalLoadUnaligned,
    /// LDMXCSR clears reserved bits instead of raising #GP(0).
    LoadMxcsrReservedNoFault,
    /// PREFETCHh faults as a load does.
    PrefetchFaults,
    /// CVTSI2SS and CVTSI2SD round to nearest whatever MXCSR says.
    IntegerToFloatRoundsToNearest,
    /// A conversion to 64 bits writes the 32-bit integer indefinite.
    FloatToIntegerIndefiniteThirtyTwo,
    /// CVTSI2SS, CVTSI2SD, ROUNDSS and ROUNDSD zero the lanes above the first.
    ScalarConversionZeroesUpperLanes,
    /// ROUND reports inexact under immediate bit 3.
    RoundReportsSuppressedInexact,
    /// ROUND takes its immediate's mode under immediate bit 2.
    RoundIgnoresMxcsrSelect,
    /// BLENDVPS, BLENDVPD and PBLENDVB select by each lane's lowest bit of XMM0.
    VariableBlendFromLowBit,
    /// MPSADBW always takes the source's first block.
    MultipleSumSourceBlockIgnored,
    /// CRC32 divides by the IEEE 802.3 polynomial.
    Crc32EthernetPolynomial,
    /// CRC32 into a 64-bit register keeps the upper half.
    Crc32KeepsUpperHalf,
    /// PCMPESTRI64 and PCMPESTRM64 read their lengths from EAX and EDX.
    StringLengthReadsLowHalf,
    /// A string compare's masked negation also negates past the second string's end.
    StringMaskedNegationWhole,
    /// A string compare's ZF reports the first string's end.
    StringZeroFlagFromFirst,
    /// PCMPESTRI and PCMPISTRI give the least significant index under immediate bit 6.
    StringIndexMostSignificantIgnored,
    /// AESENC skips MixColumns.
    AesEncryptSkipsMixColumns,
    /// AESKEYGENASSIST rotates its words left.
    AesKeygenRotatesLeft,
    /// PCLMULQDQ always takes the source's low quadword.
    CarryLessMultiplySourceSelectIgnored,
    /// SHA1RNDS4 takes the first round constant for every function.
    Sha1RoundsFirstConstant,
    /// SHA256RNDS2 does one round.
    Sha256RoundsOneRound,
    /// GF2P8MULB reduces by another polynomial.
    GaloisMultiplyWrongPolynomial,
    /// GF2P8AFFINEQB and GF2P8AFFINEINVQB take the matrix's rows in reverse.
    GaloisAffineRowsReversed,
    /// DPPS sums its products one after another.
    DotProductSumsInOrder,
    /// DPPS's first sums take each lane's own product first.
    DotProductOwnLaneFirst,
    /// MASKMOVDQU writes every byte.
    MaskedStoreWritesEveryByte,
    /// MASKMOVDQU checks its low quadword first.
    MaskedStoreLowQuadwordFirst,
    // The AVX family.
    /// VZEROUPPER zeroes the upper halves of YMM0 to YMM7 alone.
    ZeroUpperKeepsHighRegisters,
    /// VPTEST, VTESTPS, VTESTPD and VCOMISS to VUCOMISD keep the status flags they do not set.
    VexTestFlagsKeepStatus,
    /// VMOVAPS and the other aligned forms take a misaligned operand.
    VexAlignedMisalignedNoFault,
    /// The masked loads and stores check every lane's bytes, not only those of lanes the mask selects.
    MaskedAccessChecksEveryLane,
    /// The forms whose upper half reads its own immediate bits (VBLENDPS, VBLENDPD, VPBLENDD, VSHUFPD,
    /// VPERMILPD) read the lower half's.
    UpperHalfImmediateUnshifted,
    /// The broadcasts take the source's second lane.
    BroadcastSecondLane,
    /// VPMOVSX and VPMOVZX of 256 bits extend the source's low lanes into both halves.
    ExtendUpperHalfFromLowLanes,
    /// VPEXTRB to VPEXTRQ and VEXTRACTPS take lane 0 whatever the immediate.
    ExtractLaneZero,
    /// VPSLLVD to VPSRAVD shift by the count modulo the lane's width.
    VariableShiftCountWrapped,
    /// VBLENDVPS, VBLENDVPD and VPBLENDVB select by the mask lane's lowest bit.
    BlendVariableLowBit,
    /// VPERM2F128 and VPERM2I128 ignore the bits that zero a half.
    PermuteHalvesNeverZero,
    /// VPERMD and VPERMPS select by the index's low two bits.
    PermuteDoublewordsTwoBits,
    /// VPCLMULQDQ selects the second source's quadword by the immediate's bit 1.
    CarrylessSecondFromBitOne,
    /// VAESENC and VAESENCLAST skip ShiftRows.
    AesEncryptWithoutShiftRows,
    /// VMOVMSKPS, VMOVMSKPD and VPMOVMSKB of 256 bits read the lower half alone.
    SignsLowerHalfOnly,
    /// VGF2P8AFFINEQB and VGF2P8AFFINEINVQB read the matrix's rows in the other order.
    AffineMatrixRowsReversed,
    /// VCMPPS to VCMPSD read the predicate's low three bits alone.
    ComparePredicateThreeBits,
    /// The FMA forms round the product before adding.
    FusedProductRounded,
    /// VFMADDSUB and VFMSUBADD subtract in the other lanes.
    FusedAlternationSwapped,
    /// VROUNDPS to VROUNDSD report inexact whatever the immediate's bit 3.
    RoundInexactNeverSuppressed,
    /// VCVTSS2SI to VCVTTSD2SI of 64 bits give the 32-bit integer indefinite.
    ConversionToQuadwordNarrowIndefinite,
    /// VCVTSI2SS and VCVTSI2SD of a quadword convert its low doubleword.
    QuadwordToScalarLowHalf,
    /// VCVTDQ2PD and VCVTPS2PD of 256 bits convert the source's low lanes into both halves.
    WideningUpperFromLowLanes,
    /// VCVTPH2PS reads a subnormal half as zero.
    HalfSubnormalAsZero,
    /// VCVTPS2PH rounds to nearest whatever the immediate asks.
    HalfRoundsToNearest,
    /// The VEX scalar floating-point forms take the upper lanes from the second source.
    VexScalarUpperFromSecond,
    /// VDPPS and VDPPD multiply every lane whatever the immediate's high nibble.
    DotProductEveryLane,
    /// VPCMPESTRI to VPCMPISTRM ignore the masked polarities.
    StringCompareMaskedPolarityIgnored,
    /// VMASKMOVDQU stores every byte whatever its mask.
    MaskedByteStoreEveryByte,
    /// The gathers leave the mask as it was once they complete.
    GatherMaskKept,
    // The family of the legacy integer forms left, x87, MMX and the general-purpose extensions.
    /// ANDN complements its second source, not its first.
    AndNotComplementsSecond,
    /// BEXTR takes the field's length from the control's low byte.
    ExtractBitFieldLengthFromLowByte,
    /// BLSR sets CF for a zero result, not a zero source.
    ResetLowestSetBitCarryFromResult,
    /// BZHI reads its index from the whole register, not its low byte.
    ZeroHighBitsIndexWholeRegister,
    /// LZCNT sets CF for a zero count, not a zero source.
    CountCarryFromResult,
    /// PEXT packs the selected bits in reverse order.
    ExtractBitsReversed,
    /// SHLX does not mask its count.
    ShiftLeftWithoutFlagsCountUnmasked,
    /// ADOX ignores the OF it reads.
    AddWithOverflowFlagIgnoresOverflow,
    /// MULX with one register for both halves leaves the low half there.
    MultiplyWithoutFlagsLowWins,
    /// MOVBE of 32 bits into a register keeps its upper half.
    MoveSwappingBytesKeepsUpperHalf,
    /// CMPXCHG16B raises no #GP(0) for an operand not aligned to 16.
    CompareExchangePairAlignmentUnchecked,
    /// CMPXCHG8B and CMPXCHG16B compare the low halves alone.
    CompareExchangePairComparesLowHalf,
    /// CMPXCHG8B and CMPXCHG16B write EDX:EAX also when the comparison holds, clearing upper halves.
    CompareExchangePairWritesAccumulatorWhenEqual,
    /// The privileged forms raise #UD, not #GP(0).
    PrivilegedOperandFormsInvalidOpcode,
    /// INT 4 raises #GP, as if its gate were the kernel's.
    InterruptOverflowGeneralProtection,
    /// INT n's #GP error code lacks the bit that names the IDT.
    InterruptErrorCodeWithoutTableBit,
    /// A hint-space no-op reads its memory operand.
    NoOperationReadsItsOperand,
    /// CLFLUSH, CLFLUSHOPT and CLWB need their page writable.
    FlushChecksWrite,
    /// MMX forms run with an x87 exception pending.
    MmxIgnoresPendingException,
    /// EMMS marks every x87 register valid.
    EmptyStateMarksValid,
    /// MMX forms leave TOP as it was.
    MmxKeepsTop,
    /// An MMX write leaves bits 64 to 79 of its x87 register clear.
    MmxWriteKeepsExponent,
    /// PUNPCKH of MMX registers interleaves their low halves.
    MmxUnpackHighFromLowHalves,
    /// PSHUFB of MMX registers takes four bits of each index.
    MmxShuffleBytesIndexFourBits,
    /// An MMX store that faults leaves TOP as it was.
    MmxStoreFaultKeepsTop,
    /// x87 forms run with an exception pending.
    X87IgnoresPendingException,
    /// FLDCW keeps the reserved bits it loads.
    LoadControlKeepsEveryBit,
    /// x87 forms leave FIP as it was.
    X87InstructionPointerKept,
    /// A push onto a register in use raises no stack fault.
    X87PushIgnoresOverflow,
    /// FLD ST(i) reports an overflow before an underflow.
    X87OverflowBeforeUnderflow,
    /// An 80-bit operand is not checked for alignment.
    ExtendedAlignmentUnchecked,
    /// FSTP m80 stores its ten bytes at once.
    ExtendedStoreWhole,
    /// FLD m32 and m64 push despite an unmasked IE.
    LoadFloatPushesDespiteUnmasked,
    /// FCOM takes a QNaN without IE.
    OrderedComparisonQuietNaNValid,
    /// FIST rounds a half away from zero, not to even.
    RoundHalfAwayFromZero,
    /// CALL, RET and JMP m64 go to a target that is not canonical.
    StackBranchToNonCanonical,
    /// RET imm16 releases nothing beyond the return address.
    ReturnReleasesBeforePop,
    // The opmask forms and the EVEX integer forms.
    /// KADD keeps the carry out of its width.
    OpmaskAddKeepsCarry,
    /// KSHIFTL and KSHIFTR take their count modulo the width.
    OpmaskShiftCountModulo,
    /// KORTEST sets CF where any bit is set.
    OpmaskOrTestCarryOnAnyBit,
    /// KUNPCK puts the first source's half low.
    OpmaskUnpackHalvesSwapped,
    /// Zeroing masks merge.
    EvexIntegerZeroingMerges,
    /// A vector shorter than 512 bits keeps the destination's bits above it.
    EvexIntegerUpperBitsKept,
    /// ZMM16 to ZMM31 are read and written as ZMM0 to ZMM15.
    EvexIntegerHighRegistersAliasLow,
    /// A lane the writemask keeps still faults.
    EvexIntegerNoFaultSuppression,
    /// A broadcast memory operand is read whole.
    EvexIntegerBroadcastIgnored,
    /// VPTERNLOG indexes its immediate with the two sources swapped.
    TernaryLogicSourcesSwapped,
    /// VPCMP's predicate 2 (less or equal) compares less.
    CompareLessOrEqualAsLess,
    /// The per-lane shifts take their count modulo the width.
    LaneShiftCountModulo,
    /// VPROR rotates left.
    RotateRightAsLeft,
    /// VPBLENDM keeps the destination's lane, not the first source's.
    BlendKeepsDestination,
    /// A compare or test into a mask ignores its writemask.
    CompareIgnoresWritemask,
    /// RDRAND and RDSEED leave the status flags as they were.
    RandomLeavesStatusFlags,
    /// RDRAND and RDSEED clear CF, as if the random source had no value ready.
    RandomClearsCarry,
    /// A string form moves RSI and RDI up whatever DF holds.
    StringDirectionIgnored,
    /// A repeated string form leaves the instruction pointer past it after one iteration, as if it were done.
    RepeatEndsAfterOneIteration,
    /// REPE stops where the elements are equal.
    RepeatWhileEqualReversed,
    /// Under the 0x67 prefix a string form keeps the upper halves of RSI, RDI and RCX.
    StringNarrowKeepsUpperHalves,
    /// SCAS and CMPS subtract the first element from the second.
    StringCompareOperandsSwapped,
    /// MOVS checks its destination before it reads its source.
    MoveChecksDestinationFirst,
    /// A string form reads its source through DS whatever segment overrides it.
    StringSourceSegmentIgnored,
    /// A repeated string form whose count is zero still runs one iteration.
    RepeatCountZeroRuns,
    /// REPE and REPNE write the status flags after an iteration that leaves the repetition going on.
    RepeatFlagsWrittenEachIteration,
    /// CMPS reads its source element before its destination element.
    CompareReadsSourceFirst,
    /// PUSHF pushes IF clear, and SYSCALL saves it clear in R11.
    PushFlagsOmitsInterrupt,
    /// The 16-bit POPF changes the flags above bit 15 too.
    PopFlagsSixteenReachesUpper,
    /// POPF leaves AC as it was.
    PopFlagsKeepsAlignmentCheck,
    /// ENTER takes its whole nesting level rather than modulo 32.
    EnterNestingUnreduced,
    /// ENTER does not check the stack at its final RSP.
    EnterProbeSkipped,
    /// The conversions between MMX and XMM registers and MASKMOVQ run while an x87 exception is pending.
    MmxConvertIgnoresPendingException,
    /// CVTPD2PI and CVTTPD2PI from memory take a misaligned operand.
    MmxConvertMisalignedNoFault,
    /// CVTPI2PS zeroes the destination's upper quadword.
    ConvertToSinglesZeroesUpperHalf,
    /// CVTPI2PS and CVTPI2PD from memory put the x87 state in MMX's use.
    ConvertFromMemoryEntersMmx,
    /// MASKMOVQ writes every byte whatever its mask holds.
    MaskedQuadwordWritesEveryByte,
    /// The conversions that name an MMX register put the x87 state in MMX's use only once #XM cannot stop them.
    MmxConvertEntersMmxLast,
    /// Of two NaNs of one kind the x87 arithmetic gives the one with the smaller significand.
    X87NanSmallerSignificandWins,
    /// The x87 arithmetic raises no DE for a denormal operand.
    X87DenormalOperandIgnored,
    /// FSUBR and FDIVR compute as FSUB and FDIV.
    X87ReverseIgnored,
    /// FLDPI to FLDLN2 load their constants cut to 64 bits whatever the rounding.
    X87ConstantsTruncated,
    /// The x87 arithmetic rounds to 64 bits whatever FCW's precision control holds.
    X87PrecisionControlIgnored,
    /// The x87 arithmetic and FST to memory find a result tiny where it lies below the smallest normal before rounding.
    X87TinyBeforeRounding,
    /// FPREM and FPREM1 complete the remainder whatever the exponents' difference.
    X87RemainderNeverPartial,
    /// FXSAVE and FXRSTOR take an area not aligned to 16.
    X87AreaMisalignedNoFault,
    /// FXRSTOR loads an MXCSR with reserved bits set.
    X87RestoreTakesReservedMxcsr,
    /// FNSTENV and FNSAVE leave the 16-bit environment's selector words as they were.
    X87EnvironmentSkipsSelectors,
    /// FNSTENV to FRSTOR take a misaligned area under AC.
    X87EnvironmentAlignmentUnchecked,
    /// FXSAVE and FXRSTOR report a page fault from the area's first byte.
    X87AreaFaultsAtFirstByte,
    /// LAR to VERW read a selector's table indicator as if it named the GDT.
    DescriptorTableIndicatorIgnored,
    /// LAR to VERW fail a selector whose RPL is below 3.
    DescriptorRequestedPrivilegeChecked,
    /// LSL gives the limit in pages.
    SegmentLimitInPages,
    /// VERW succeeds for a readable code segment.
    VerifyWriteTakesCode,
    /// SYSCALL writes its own address into RCX.
    SystemCallReturnsToItself,
    /// RCP reads the table's entry after the source's.
    ReciprocalTableNeighbor,
    /// RSQRT reads an odd exponent's entries for an even exponent and the other way round.
    RootParitySwapped,
    /// IRET runs with NT set.
    InterruptReturnIgnoresNestedTask,
    /// Loading FS or GS keeps its base.
    SegmentLoadKeepsBase,
    /// Loading a null selector into FS or GS keeps its base.
    NullLoadKeepsBase,
    /// A far RET or IRET takes a CS whose RPL is below 3.
    ReturnToInnerPrivilegeAllowed,
    /// A far transfer takes a target outside its code segment.
    FarTargetUnchecked,
    /// POP FS and POP GS read their whole slot.
    SelectorPopReadsWholeSlot,
    /// A transfer into compatibility mode keeps RSP whole.
    CompatibilityKeepsWholeStackPointer,
    /// A far return and IRET read their slots from the lowest.
    StackSlotsReadInOrder,
    /// The far CALL checks its target before it pushes.
    CallFarChecksTargetFirst,
    /// VMCALL under KVM answers 0.
    HypervisorCallAnswersZero,
    /// WRPKRU takes a nonzero EDX.
    WritePkruIgnoresEdx,
    /// A VEX write of a vector register keeps ZMM's bits 256 to 511.
    VexKeepsZmmUpperBits,
    /// VZEROUPPER keeps ZMM's bits 256 to 511.
    ZeroUpperKeepsZmmUpperBits,
    /// VZEROUPPER and VZEROALL zero ZMM16 to ZMM31 too.
    ZeroAllClearsHighRegisters,
    /// VPDPBUSDS and VPDPWSSDS wrap rather than saturate.
    DotSaturationWraps,
    /// VPDPBUSD and VPDPBUSDS take the first source's bytes as signed.
    DotUnsignedBytesSigned,
    /// VPDPBUSDS and VPDPWSSDS wrap the products' sum to 32 bits before adding the destination.
    DotProductsWrappedFirst,
    /// XSAVES and XRSTORS check their memory operand's page before the privilege level.
    SupervisorSaveChecksOperandFirst,
    /// VPADDS, VPADDUS, VPSUBS and VPSUBUS wrap rather than saturate.
    EvexSaturationWraps,
    /// VPAVGB and VPAVGW halve the sum without adding one.
    EvexAverageTruncates,
    /// VPMADDUBSW wraps its word sums rather than saturating them.
    EvexMultiplyAddBytesWraps,
    /// The packs saturate as if signed whatever the form.
    EvexPackAlwaysSigned,
    /// The unpacks take the second source's lane first.
    EvexIntegerUnpackSourcesSwapped,
    /// VPSLLDQ and VPSRLDQ take their count modulo 16.
    EvexByteShiftCountModulo,
    /// VPCMPB, VPCMPW, VPCMPUB and VPCMPUW compare unsigned as signed and signed as unsigned.
    EvexByteWordCompareSignednessSwapped,
    /// A broadcast repeats a lane of the destination's width, not the source's.
    EvexBroadcastDestinationLane,
    /// A tuple broadcast repeats its first lane alone.
    EvexBroadcastTupleFirstLane,
    /// VMOVSHDUP duplicates the even singles.
    EvexDuplicateOddAsEven,
    /// A form whose memory lanes are not the destination's reads only the lanes its writemask lets through.
    EvexWholeReadSuppressed,
    /// VANDNPS, VANDNPD, VPANDND and VPANDNQ complement the second source.
    EvexAndNotComplementsSecond,
    /// VPMOVM2B to VPMOVM2Q set the lanes whose opmask bit is clear.
    EvexFromMaskInverted,
    /// VPMOVB2M to VPMOVQ2M take each lane's lowest bit.
    EvexToMaskLowestBit,
    /// VPMOVSX extends with zeros.
    EvexExtendSignedWithZeros,
    /// The saturating narrowing forms truncate.
    EvexNarrowTruncates,
    /// VDBPSADBW ignores its immediate, taking the second source's doublewords in order.
    EvexBlockDifferencesInOrder,
    /// VINSERTI32X4 to VINSERTF64X4 replace the lowest part whatever the immediate.
    EvexInsertLowestPart,
    /// VEXTRACTI32X4 to VEXTRACTF64X4 read the immediate's every bit, past the parts there are.
    EvexExtractIndexUnreduced,
    /// VMOVHLPS moves the low quadword high, and VMOVLHPS, VMOVHPS and VMOVHPD the high one low.
    EvexHalvesMovedTheOtherWay,
    /// VPINSRB to VPINSRQ replace the lowest lane whatever the immediate.
    EvexInsertLaneLowest,
    /// VMOVD and VMOVQ into an XMM register keep its other lanes.
    EvexMoveLowKeepsOthers,
    // The second EVEX pass.
    /// DAZ reads a half's denormal as zero.
    HalfDenormalsAreZeros,
    /// FTZ flushes a tiny half to zero.
    HalfFlushesToZero,
    /// An unmasked underflow to a half reports inexact as SSE does, from the result with no bound on the exponent.
    HalfUnderflowInexactUnbounded,
    /// An integer that overflows a half is not reported inexact unless the overflow is masked.
    IntegerToHalfOverflowExact,
    /// The EVEX VCVTPH2PS reports a denormal half, as VCVTPH2PSX does.
    HalfToSingleReportsDenormal,
    /// A conversion to an unsigned integer gives the signed integer indefinite, the sign bit alone.
    EvexUnsignedIndefiniteSigned,
    /// VEXPAND takes the moved elements from the highest of those the source holds.
    EvexExpandReadsFromTheTop,
    /// VCOMPRESS and VEXPAND with memory reach the whole vector's bytes, not only the moved elements'.
    EvexCompressReachesWholeVector,
    /// VGETMANT's interval 3/4 to 3/2 halves the significand when its fraction's top bit is clear.
    MantissaIntervalThreeInverted,
    /// VSCALEF truncates its power toward zero rather than taking its floor.
    ScaleTruncatesPower,
    /// VFPCLASS's negative finite class takes in -0.
    ClassNegativeFiniteIncludesZero,
    /// VRNDSCALE and VREDUCE report inexact even where the immediate's bit 3 suppresses it.
    RoundScaleSuppressionIgnored,
    /// VDPBF16PS adds the lower pair's product first.
    DotBfloat16LowerPairFirst,
    /// VDPBF16PS keeps the sum's NaN over a source's.
    DotBfloat16SumNanFirst,
    /// XEND outside a transaction completes as no operation.
    TransactionEndCompletes,
    /// XTEST outside a transaction clears ZF, as inside one.
    TransactionTestClearsZero,
    /// VFIXUPIMM takes the token of +1 for -1 as well.
    FixupOneIgnoresSign,
    /// VFMULCPH and VFCMULCPH (and their accumulating forms) swap which part's second product is negated.
    ComplexConjugateSignsSwapped,
    /// The complex multiplies raise #XM for an unmasked exception, as the other arithmetic does.
    ComplexHonorsMasks,
    /// The EVEX VCVTPS2PH rounds as MXCSR says, whatever its immediate.
    HalfConversionImmediateIgnored,
    /// The EVEX gathers and scatters ignore the scale.
    GatherScaleIgnored,
    /// An EVEX gather that faults leaves the opmask as it was, its bits beyond the elements included, cleared.
    GatherFaultClearsMask,
    /// An EVEX scatter leaves the opmask as it was once it completes.
    ScatterMaskKept,
    /// A VMOVSH load and VMOVW into a vector keep the destination's bits above the word up to 128.
    MoveWordKeepsUpperBits,
    /// VRANGE's sign control 0 keeps the selected element's sign rather than the first source's.
    RangeSignFromSelected,
    // The optional forms left and the generator's holes.
    /// FISTTP rounds by FCW's rounding control, as FISTP does.
    TruncatingStoreRoundsByControl,
    /// SSSE3's MMX forms read 16 bytes of memory, as their XMM forms do.
    MmxSsseThreeMemoryReadsSixteenBytes,
    /// EVEX VPEXTR, VEXTRACTPS take the immediate's low four bits as the index, whatever the element's size.
    ExtractIndexUnmasked,
    /// EVEX VMOVHPS and VMOVLPS store the other quadword.
    QuadwordStoreHalvesSwapped,
    /// The family's EVEX forms ignore the writemask.
    OptionalMaskIgnored,
    /// EVEX VMOVSS and VMOVSD of registers keep the destination's bits 128 to 255.
    ScalarMoveKeepsUpperBits,
    /// EVEX VCVTPS2PH to memory stores despite an unmasked precision, underflow or overflow.
    HalfStoreExceptionsIgnored,
    /// VEX VRCPPS and VRSQRTPS of an XMM register keep the destination's bits 128 to 255.
    VexReciprocalKeepsUpperHalf,
    /// A masked memory source faults on elements the writemask leaves out.
    OptionalMaskedFaultNotSuppressed,
    /// VRCP14 and VRCPPH take a denormal source as zero whatever DAZ holds.
    ReciprocalDenormalsAreZeros,
    /// VRCP14 reads its table by the source's top 15 significand bits.
    ReciprocalFourteenIndexFifteenBits,
    /// VRSQRT14 and VRSQRTPH read their table by the wrong exponent parity.
    RootReciprocalParitySwapped,
    /// WRFSBASE and WRGSBASE take a non-canonical base.
    BaseWriteCanonicalUnchecked,
    /// UMONITOR faults as a read of one byte would.
    MonitorChecksRead,
    /// EVEX VCVTPS2PH to memory leaves MXCSR's flags as they were when its store faults.
    HalfStoreFaultClearsFlags,
    /// A masked EVEX VCVTPS2PH to memory reports the first byte that faults, as an unmasked store does.
    HalfStoreFaultAtFirstByte,
    /// MOVDIR64B takes a destination that does not lie on 64 bytes.
    BlockStoreAlignmentUnchecked,
    /// CMPS and SCAS take AF from the carry out of bit 2, bit 3 of the operands and difference joined.
    StringCompareAuxiliaryFromBitThree,
    /// SUB, CMP and SBB take AF from bit 3 of the operands and difference joined.
    SubtractAuxiliaryFromBitThree,
    /// NEG takes AF from bit 3 of the operand and result joined.
    NegateAuxiliaryFromBitThree,
    /// A VEX instruction keeps ZMM's bits 256 to 511 of a destination it also reads.
    VexReadDestinationKeepsZmmUpperBits,
    /// XGETBV with ECX 1 gives XCR0 whole.
    ControlReadIgnoresInUse,
    /// XGETBV with ECX 1 sets SSE's bit from XINUSE alone, whatever MXCSR holds.
    ControlReadIgnoresMxcsr,
    /// XSAVE and XSAVEOPT clear the header's bits outside the request.
    SaveHeaderDropsUnrequested,
    /// XSAVE and XSAVEOPT save MXCSR only when SSE is requested, not for AVX alone.
    SaveMxcsrOnlyWithSse,
    /// XSAVEOPT saves the components in their initial configuration too.
    OptimizedSavesInitial,
    /// XSAVEC leaves SSE's bit to XINUSE whatever MXCSR holds.
    CompactedIgnoresMxcsr,
    /// XSAVEC puts each component at its standard offset.
    CompactedStandardOffsets,
    /// The family's request takes EAX as its upper half and EDX as its lower.
    SaveRequestHalvesSwapped,
    /// XSAVE, XSAVEC and XSAVEOPT without REX.W save FIP and FDP whole.
    NarrowSaveWidePointers,
    /// The family's area needs only 16-byte alignment.
    AreaAlignedToSixteen,
    /// XRSTOR takes an XSTATE_BV with a bit outside XCR0.
    RestoreAcceptsUnenabledComponents,
    /// XRSTOR loads MXCSR only when SSE is requested.
    RestoreMxcsrOnlyWithSse,
    /// The family's page fault beyond the area's first page is reported at the lowest byte that faults.
    AreaFaultsAtLowestByte,
    /// VZEROUPPER tracks SSE as initial too.
    ZeroUpperTracksSseInitial,
    /// XRSTOR reads its header from XSTATE_BV on, so that a fault there is reported at the header's start.
    RestoreReadsHeaderFromStart,
    /// XRSTOR tracks the components it restores as modified.
    RestoreTracksModified,
    // The x87 transcendental forms, JCXZ and the forms the chip source refused.
    /// FPTAN pushes +0 where it pushes 1.0.
    TangentPushesZero,
    /// FSINCOS leaves the cosine in ST(1) and pushes the sine.
    SineCosineOrderSwapped,
    /// The trigonometric forms take arguments up to 2 to the 64 as in range.
    TrigonometricRangeTo64,
    /// FYL2X of a zero x raises no ZE.
    LogOfZeroNoDivideByZero,
    /// FPATAN of ±0 over -0 gives ±0, not ±π.
    ArcTangentZeroOverNegativeZero,
    /// F2XM1 beyond ±1 raises no PE.
    TwoToXOutOfRangeExact,
    /// FYL2XP1 of an x at -1 or below raises IE.
    LogOnePlusBelowMinusOneInvalid,
    /// The transcendental forms raise no DE for a denormal operand.
    TranscendentalDenormalIgnored,
    /// FPATAN, FYL2X and FYL2XP1 do not pop.
    BinaryTranscendentalKeepsStack,
    /// A binary transcendental form's behavior leaves ST(0) undecided, not the result's ST(1).
    UndecidedRegisterMisplaced,
    /// SYSENTER leaves the instruction pointer past it, as SYSCALL does.
    SystemEnterAdvances,
    /// RDTSC and RDTSCP read the counter whatever CR4.TSD holds.
    TimeStampCounterUnrestricted,
    /// UMWAIT and TPAUSE wait whatever CR4.TSD holds.
    WaitWithoutTimeStampControl,
    /// JCXZ tests ECX, as JECXZ does.
    CountJumpTestsDoubleword,
    /// JCXZ under the prefix 0x66 keeps the target's bits above 16.
    SixteenBitJumpTargetWhole,
    /// A step in compatibility mode keeps RSP's upper half.
    CompatibilityStepKeepsStackUpperHalf,
}

const fn row(bug: Bug, family_name: &'static str, family: Family, origin: Origin) -> PlantedBug {
    PlantedBug { bug, family_name, family, origin }
}

const fn integer(test: fn(Operation) -> bool) -> Family {
    Family::Integer(test)
}

const fn sse(test: fn(SseOperation) -> bool) -> Family {
    Family::Sse(test)
}

use Origin::{
    Avx512Integer, EvexSecondHalf, FifthCheck, FifthPass, FirstCheck, FirstPass, FourthCheck, FourthPass, Port, SecondCheck,
    SecondPass, SseFamily, ThirdCheck, ThirdPass, WholeList,
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
    row(Bug::LaneSelectorUnmasked, "lane moves by an immediate", sse(|o| matches!(o, SseOperation::Extract { select: LaneSelect::Immediate, .. } | SseOperation::Insert { select: LaneSelect::Immediate, .. })), SseFamily),
    row(Bug::HighQuadwordStoreReadsLow, "MOVHPS, MOVHPD", Family::Forms(&[Instruction::MovhpsMemqXmmps, Instruction::MovhpdMemqXmmsd]), SseFamily),
    row(Bug::ExtractKeepsUpperHalf, "lane moves to a general register", sse(|o| matches!(o, SseOperation::Extract { .. })), SseFamily),
    row(Bug::InsertZeroingKeepsUpperLanes, "zeroing moves into XMM", sse(|o| matches!(o, SseOperation::Insert { zeroing: true, .. })), SseFamily),
    row(Bug::MoveMaskFromLowBits, "MOVMSKPS, MOVMSKPD, PMOVMSKB", sse(|o| matches!(o, SseOperation::MoveMask(_))), SseFamily),
    row(Bug::AlignedStoreMisalignedNoFault, "MovapsMempsXmmps", Family::Forms(&[Instruction::MovapsMempsXmmps]), SseFamily),
    row(Bug::NonTemporalLoadUnaligned, "MovntdqaXmmdqMemdq", Family::Forms(&[Instruction::MovntdqaXmmdqMemdq]), SseFamily),
    row(Bug::LoadMxcsrReservedNoFault, "LdmxcsrMemd", Family::Forms(&[Instruction::LdmxcsrMemd]), SseFamily),
    row(Bug::PrefetchFaults, "PREFETCHh", sse(|o| o == SseOperation::Prefetch), SseFamily),
    row(Bug::IntegerToFloatRoundsToNearest, "CVTSI2SS, CVTSI2SD", sse(|o| matches!(o, SseOperation::IntegerToFloat(_))), SseFamily),
    row(Bug::FloatToIntegerIndefiniteThirtyTwo, "conversions to integers", sse(|o| matches!(o, SseOperation::FloatToInteger { .. })), SseFamily),
    row(Bug::ScalarConversionZeroesUpperLanes, "scalar conversions and rounds", sse(|o| matches!(o, SseOperation::IntegerToFloat(_) | SseOperation::Round { arrangement: Arrangement::Scalar, .. })), SseFamily),
    row(Bug::RoundReportsSuppressedInexact, "ROUND", sse(|o| matches!(o, SseOperation::Round { .. })), SseFamily),
    row(Bug::RoundIgnoresMxcsrSelect, "ROUND", sse(|o| matches!(o, SseOperation::Round { .. })), SseFamily),
    row(Bug::VariableBlendFromLowBit, "variable blends", sse(|o| matches!(o, SseOperation::Combine(Combination::VariableBlend(_)))), SseFamily),
    row(Bug::MultipleSumSourceBlockIgnored, "MPSADBW", sse(|o| o == SseOperation::Combine(Combination::MultipleSumAbsoluteDifferences)), SseFamily),
    row(Bug::Crc32EthernetPolynomial, "CRC32", sse(|o| o == SseOperation::Crc32), SseFamily),
    row(Bug::Crc32KeepsUpperHalf, "CRC32", sse(|o| o == SseOperation::Crc32), SseFamily),
    row(Bug::StringLengthReadsLowHalf, "string compares", sse(|o| matches!(o, SseOperation::CompareStrings { .. })), SseFamily),
    row(Bug::StringMaskedNegationWhole, "string compares", sse(|o| matches!(o, SseOperation::CompareStrings { .. })), SseFamily),
    row(Bug::StringZeroFlagFromFirst, "string compares", sse(|o| matches!(o, SseOperation::CompareStrings { .. })), SseFamily),
    row(Bug::StringIndexMostSignificantIgnored, "string compares", sse(|o| matches!(o, SseOperation::CompareStrings { .. })), SseFamily),
    row(Bug::AesEncryptSkipsMixColumns, "AES", sse(|o| matches!(o, SseOperation::Combine(Combination::Aes(_)))), SseFamily),
    row(Bug::AesKeygenRotatesLeft, "AES", sse(|o| matches!(o, SseOperation::Combine(Combination::Aes(_)))), SseFamily),
    row(Bug::CarryLessMultiplySourceSelectIgnored, "PCLMULQDQ", sse(|o| o == SseOperation::Combine(Combination::CarryLessMultiply)), SseFamily),
    row(Bug::Sha1RoundsFirstConstant, "SHA", sse(|o| matches!(o, SseOperation::Combine(Combination::Sha(_)))), SseFamily),
    row(Bug::Sha256RoundsOneRound, "SHA", sse(|o| matches!(o, SseOperation::Combine(Combination::Sha(_)))), SseFamily),
    row(Bug::GaloisMultiplyWrongPolynomial, "GFNI", sse(|o| matches!(o, SseOperation::Combine(Combination::Galois(_)))), SseFamily),
    row(Bug::GaloisAffineRowsReversed, "GFNI", sse(|o| matches!(o, SseOperation::Combine(Combination::Galois(_)))), SseFamily),
    row(Bug::DotProductSumsInOrder, "DPPS", sse(|o| o == SseOperation::DotProduct(Precision::Single)), SseFamily),
    row(Bug::DotProductOwnLaneFirst, "DPPS", sse(|o| o == SseOperation::DotProduct(Precision::Single)), SseFamily),
    row(Bug::MaskedStoreWritesEveryByte, "MASKMOVDQU", sse(|o| o == SseOperation::MaskedStore), SseFamily),
    row(Bug::MaskedStoreLowQuadwordFirst, "MASKMOVDQU", sse(|o| o == SseOperation::MaskedStore), SseFamily),
    row(Bug::ZeroUpperKeepsHighRegisters, "VZEROUPPER", Family::Avx(|form| form.operation == AvxOperation::ZeroUpper), Origin::Avx),
    row(Bug::VexTestFlagsKeepStatus, "VPTEST, VTESTPS, VCOMISS", Family::Avx(|form| form.operation.writes_flags()), Origin::Avx),
    row(Bug::VexAlignedMisalignedNoFault, "VMOVAPS and the aligned forms", Family::Avx(|form| form.aligned), Origin::Avx),
    row(Bug::MaskedAccessChecksEveryLane, "VMASKMOVPS, VPMASKMOVD", Family::Avx(|form| matches!(form.operation, AvxOperation::MaskedLoad(_) | AvxOperation::MaskedStore(_))), Origin::Avx),
    row(Bug::UpperHalfImmediateUnshifted, "VBLENDPS, VSHUFPD", Family::Avx(|form| form.wide && matches!(form.operation, AvxOperation::Halves { upper_shift: 1.., .. })), Origin::Avx),
    row(Bug::BroadcastSecondLane, "VBROADCASTSS, VPBROADCASTB", Family::Avx(|form| matches!(form.operation, AvxOperation::Broadcast(_))), Origin::Avx),
    row(Bug::ExtendUpperHalfFromLowLanes, "VPMOVSX, VPMOVZX of 256 bits", Family::Avx(|form| form.wide && matches!(form.operation, AvxOperation::Extend(_))), Origin::Avx),
    row(Bug::ExtractLaneZero, "VPEXTRB, VEXTRACTPS", Family::Avx(|form| matches!(form.operation, AvxOperation::ExtractLane(_))), Origin::Avx),
    row(Bug::VariableShiftCountWrapped, "VPSLLVD, VPSRAVD", Family::Avx(|form| matches!(form.operation, AvxOperation::ShiftEach { .. })), Origin::Avx),
    row(Bug::BlendVariableLowBit, "VBLENDVPS, VPBLENDVB", Family::Avx(|form| matches!(form.operation, AvxOperation::BlendVariable(_))), Origin::Avx),
    row(Bug::PermuteHalvesNeverZero, "VPERM2F128", Family::Avx(|form| form.operation == AvxOperation::PermuteHalves), Origin::Avx),
    row(Bug::PermuteDoublewordsTwoBits, "VPERMD, VPERMPS", Family::Avx(|form| form.operation == AvxOperation::PermuteDoublewords), Origin::Avx),
    row(Bug::CarrylessSecondFromBitOne, "VPCLMULQDQ", Family::Avx(|form| form.operation == AvxOperation::CarrylessMultiply), Origin::Avx),
    row(Bug::AesEncryptWithoutShiftRows, "VAESENC, VAESENCLAST", Family::Avx(|form| matches!(form.operation, AvxOperation::Aes(crate::avx::AesStep::Encrypt | crate::avx::AesStep::EncryptLast))), Origin::Avx),
    row(Bug::SignsLowerHalfOnly, "VMOVMSKPS of 256 bits", Family::Avx(|form| form.wide && matches!(form.operation, AvxOperation::Signs(_))), Origin::Avx),
    row(Bug::AffineMatrixRowsReversed, "VGF2P8AFFINEQB", Family::Avx(|form| matches!(form.operation, AvxOperation::GaloisAffine { .. })), Origin::Avx),
    row(Bug::ComparePredicateThreeBits, "VCMPPS", Family::Avx(|form| matches!(form.operation, AvxOperation::Compare { .. })), Origin::Avx),
    row(Bug::FusedProductRounded, "FMA", Family::Avx(|form| matches!(form.operation, AvxOperation::Fused { .. })), Origin::Avx),
    row(Bug::FusedAlternationSwapped, "VFMADDSUB, VFMSUBADD", Family::Avx(|form| matches!(form.operation, AvxOperation::Fused { sign: crate::avx::FusedSign::MultiplyAddSubtract | crate::avx::FusedSign::MultiplySubtractAdd, .. })), Origin::Avx),
    row(Bug::RoundInexactNeverSuppressed, "VROUNDPS", Family::Avx(|form| matches!(form.operation, AvxOperation::RoundToIntegral { .. })), Origin::Avx),
    row(Bug::ConversionToQuadwordNarrowIndefinite, "VCVTSS2SI of 64 bits", Family::Avx(|form| matches!(form.operation, AvxOperation::ScalarToInteger { .. }) && form.code.op_code().op_kind(0) == iced_x86::OpCodeOperandKind::r64_reg), Origin::Avx),
    row(Bug::QuadwordToScalarLowHalf, "VCVTSI2SS of 64 bits", Family::Avx(|form| matches!(form.operation, AvxOperation::IntegerToScalar(_)) && form.code.op_code().op_kind(2) == iced_x86::OpCodeOperandKind::r64_or_mem), Origin::Avx),
    row(Bug::WideningUpperFromLowLanes, "VCVTPS2PD of 256 bits", Family::Avx(|form| form.wide && matches!(form.operation, AvxOperation::ConvertWidening(_))), Origin::Avx),
    row(Bug::HalfSubnormalAsZero, "VCVTPH2PS", Family::Avx(|form| form.operation == AvxOperation::HalfToSingle), Origin::Avx),
    row(Bug::HalfRoundsToNearest, "VCVTPS2PH", Family::Avx(|form| form.operation == AvxOperation::SingleToHalf), Origin::Avx),
    row(Bug::DotProductEveryLane, "VDPPS, VDPPD", Family::Avx(|form| matches!(form.operation, AvxOperation::DotProduct(_))), Origin::Avx),
    row(Bug::StringCompareMaskedPolarityIgnored, "VPCMPESTRI, VPCMPISTRM", Family::Avx(|form| matches!(form.operation, AvxOperation::StringCompare { .. })), Origin::Avx),
    row(Bug::MaskedByteStoreEveryByte, "VMASKMOVDQU", Family::Avx(|form| form.operation == AvxOperation::MaskedByteStore), Origin::Avx),
    row(Bug::GatherMaskKept, "VGATHERDPS, VPGATHERQQ", Family::Avx(|form| matches!(form.operation, AvxOperation::Gather { .. })), Origin::Avx),
    row(Bug::VexScalarUpperFromSecond, "VADDSS and the VEX scalar forms", Family::Avx(|form| matches!(form.operation, AvxOperation::Float { arrangement: crate::float::Arrangement::Scalar, operation, .. } if !operation.writes_flags())), Origin::Avx),
    row(Bug::AndNotComplementsSecond, "ANDN", Family::Forms(ANDN), Origin::IntegerX87Mmx),
    row(Bug::ExtractBitFieldLengthFromLowByte, "BEXTR", Family::Forms(BEXTR), Origin::IntegerX87Mmx),
    row(Bug::ResetLowestSetBitCarryFromResult, "BLSR", Family::Forms(BLSR), Origin::IntegerX87Mmx),
    row(Bug::ZeroHighBitsIndexWholeRegister, "BZHI", Family::Forms(BZHI), Origin::IntegerX87Mmx),
    row(Bug::CountCarryFromResult, "LZCNT", Family::Forms(LZCNT), Origin::IntegerX87Mmx),
    row(Bug::ExtractBitsReversed, "PEXT", Family::Forms(PEXT), Origin::IntegerX87Mmx),
    row(Bug::ShiftLeftWithoutFlagsCountUnmasked, "SHLX", Family::Forms(SHLX), Origin::IntegerX87Mmx),
    row(Bug::AddWithOverflowFlagIgnoresOverflow, "ADOX", Family::Forms(ADOX), Origin::IntegerX87Mmx),
    row(Bug::MultiplyWithoutFlagsLowWins, "MULX", Family::Forms(MULX), Origin::IntegerX87Mmx),
    row(Bug::MoveSwappingBytesKeepsUpperHalf, "MOVBE", Family::Forms(MOVBE), Origin::IntegerX87Mmx),
    row(Bug::CompareExchangePairAlignmentUnchecked, "CMPXCHG16B", Family::Forms(CMPXCHG16B), Origin::IntegerX87Mmx),
    row(Bug::CompareExchangePairComparesLowHalf, "CMPXCHG8B, CMPXCHG16B", Family::Forms(CMPXCHG_PAIR), Origin::IntegerX87Mmx),
    row(Bug::CompareExchangePairWritesAccumulatorWhenEqual, "CMPXCHG8B", Family::Forms(CMPXCHG8B), Origin::IntegerX87Mmx),
    row(Bug::PrivilegedOperandFormsInvalidOpcode, "LGDT, MOV CR, IN, OUT", Family::Forms(PRIVILEGED_OPERAND_FORMS), Origin::IntegerX87Mmx),
    row(Bug::InterruptOverflowGeneralProtection, "INT n", Family::Forms(&[Instruction::IntImmb]), Origin::IntegerX87Mmx),
    row(Bug::InterruptErrorCodeWithoutTableBit, "INT n", Family::Forms(&[Instruction::IntImmb]), Origin::IntegerX87Mmx),
    row(Bug::NoOperationReadsItsOperand, "hint-space no-ops", Family::Forms(HINT_NO_OPERATIONS), Origin::IntegerX87Mmx),
    row(Bug::FlushChecksWrite, "CLFLUSH, CLFLUSHOPT, CLWB", Family::Forms(FLUSHES), Origin::IntegerX87Mmx),
    row(Bug::MmxIgnoresPendingException, "MMX", Family::Forms(MMX_SAMPLE), Origin::IntegerX87Mmx),
    row(Bug::EmptyStateMarksValid, "EMMS", Family::Forms(&[Instruction::Emms]), Origin::IntegerX87Mmx),
    row(Bug::MmxKeepsTop, "MMX", Family::Forms(MMX_SAMPLE), Origin::IntegerX87Mmx),
    row(Bug::MmxWriteKeepsExponent, "MMX", Family::Forms(MMX_SAMPLE), Origin::IntegerX87Mmx),
    row(Bug::MmxUnpackHighFromLowHalves, "PUNPCKH of MMX", Family::Forms(&[Instruction::PunpckhbwMmxqMmxd, Instruction::PunpckhdqMmxqMemq]), Origin::IntegerX87Mmx),
    row(Bug::MmxShuffleBytesIndexFourBits, "PSHUFB of MMX", Family::Forms(&[Instruction::PshufbMmxqMmxq]), Origin::IntegerX87Mmx),
    row(Bug::MmxStoreFaultKeepsTop, "MMX stores", Family::Forms(&[Instruction::MovqMemqMmxq0f7f, Instruction::MovntqMemqMmxq]), Origin::IntegerX87Mmx),
    row(Bug::X87IgnoresPendingException, "x87", Family::Forms(X87_SAMPLE), Origin::IntegerX87Mmx),
    row(Bug::LoadControlKeepsEveryBit, "FLDCW", Family::Forms(&[Instruction::FldcwMemmem16]), Origin::IntegerX87Mmx),
    row(Bug::X87InstructionPointerKept, "x87", Family::Forms(X87_SAMPLE), Origin::IntegerX87Mmx),
    row(Bug::X87OverflowBeforeUnderflow, "FLD ST(i)", Family::Forms(&[Instruction::FldSt0X87]), Origin::IntegerX87Mmx),
    row(Bug::LoadFloatPushesDespiteUnmasked, "FLD m32, m64", Family::Forms(&[Instruction::FldSt0Memmem32real, Instruction::FldSt0Memm64real]), Origin::IntegerX87Mmx),
    row(Bug::OrderedComparisonQuietNaNValid, "FCOM", Family::Forms(&[Instruction::FcomSt0X87, Instruction::FcomSt0Memm64real, Instruction::Ftst]), Origin::IntegerX87Mmx),
    row(Bug::StackBranchToNonCanonical, "CALL, RET, JMP m64", Family::Forms(&[Instruction::CallNearGprv, Instruction::RetNear, Instruction::JmpMemv]), Origin::IntegerX87Mmx),
    row(Bug::ReturnReleasesBeforePop, "RET imm16", Family::Forms(&[Instruction::RetNearImmw]), Origin::IntegerX87Mmx),
    row(Bug::RoundHalfAwayFromZero, "FIST", Family::Forms(&[Instruction::FistMemmem16intSt0, Instruction::FistpMemm64intSt0]), Origin::IntegerX87Mmx),
    row(Bug::ExtendedStoreWhole, "FSTP m80", Family::Forms(&[Instruction::FstpMemmem80realSt0]), Origin::IntegerX87Mmx),
    row(Bug::ExtendedAlignmentUnchecked, "FLD m80, FSTP m80", Family::Forms(&[Instruction::FldSt0Memmem80real, Instruction::FstpMemmem80realSt0]), Origin::IntegerX87Mmx),
    row(Bug::X87PushIgnoresOverflow, "x87 pushes", Family::Forms(&[Instruction::FldSt0X87, Instruction::Fld1, Instruction::FildSt0Memmem16int]), Origin::IntegerX87Mmx),
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
    row(Bug::OpmaskAddKeepsCarry, "KADD", Family::Opmask(|o| o == OpmaskOperation::Add), Avx512Integer),
    row(Bug::OpmaskShiftCountModulo, "KSHIFTL, KSHIFTR", Family::Opmask(|o| matches!(o, OpmaskOperation::ShiftLeft | OpmaskOperation::ShiftRight)), Avx512Integer),
    row(Bug::OpmaskOrTestCarryOnAnyBit, "KORTEST", Family::Opmask(|o| o == OpmaskOperation::OrTest), Avx512Integer),
    row(Bug::OpmaskUnpackHalvesSwapped, "KUNPCK", Family::Opmask(|o| o == OpmaskOperation::Unpack), Avx512Integer),
    row(Bug::EvexIntegerZeroingMerges, "EVEX integer forms", Family::EvexInteger(|form| !form.writes_mask()), Avx512Integer),
    row(Bug::EvexIntegerUpperBitsKept, "EVEX integer forms", Family::EvexInteger(|form| !form.writes_mask()), Avx512Integer),
    row(Bug::EvexIntegerHighRegistersAliasLow, "EVEX integer forms", Family::EvexInteger(|_| true), Avx512Integer),
    row(Bug::EvexIntegerNoFaultSuppression, "EVEX integer forms in memory", Family::EvexInteger(|form| matches!(form.place, EvexIntegerPlace::Memory | EvexIntegerPlace::Store)), Avx512Integer),
    row(Bug::EvexIntegerBroadcastIgnored, "EVEX integer forms that broadcast", Family::EvexInteger(|form| form.place == EvexIntegerPlace::Memory && form.can_broadcast()), Avx512Integer),
    row(Bug::TernaryLogicSourcesSwapped, "VPTERNLOG", Family::EvexInteger(|form| form.operation == EvexIntegerOperation::TernaryLogic), Avx512Integer),
    row(Bug::CompareLessOrEqualAsLess, "VPCMP, VPCMPU", Family::EvexInteger(|form| matches!(form.operation, EvexIntegerOperation::CompareIntoMask(ComparePredicate::Immediate(_)))), Avx512Integer),
    row(Bug::LaneShiftCountModulo, "VPSLLV, VPSRLV, VPSRAV", Family::EvexInteger(|form| matches!(form.operation, EvexIntegerOperation::ShiftByLane(_))), Avx512Integer),
    row(Bug::RotateRightAsLeft, "VPROR, VPRORV", Family::EvexInteger(|form| matches!(form.operation, EvexIntegerOperation::Rotate { left: false, .. })), Avx512Integer),
    row(Bug::BlendKeepsDestination, "VPBLENDM", Family::EvexInteger(|form| form.operation == EvexIntegerOperation::Blend), Avx512Integer),
    row(Bug::CompareIgnoresWritemask, "compares and tests into masks", Family::EvexInteger(EvexIntegerForm::writes_mask), Avx512Integer),
    row(Bug::RandomLeavesStatusFlags, "RDRAND, RDSEED", Family::Forms(RANDOM), Origin::Expectations),
    row(Bug::RandomClearsCarry, "RDRAND, RDSEED", Family::Forms(RANDOM), Origin::Expectations),
    row(Bug::StringDirectionIgnored, "string forms", Family::String(|form| form.reads_source() || form.reaches_destination()), Origin::Mandatory),
    row(Bug::RepeatEndsAfterOneIteration, "repeated string forms", Family::String(|form| form.is_repeated()), Origin::Mandatory),
    row(Bug::RepeatWhileEqualReversed, "REPE SCAS, REPE CMPS", Family::String(|form| form.repetition == crate::string::Repetition::WhileEqual), Origin::Mandatory),
    row(Bug::StringNarrowKeepsUpperHalves, "string forms", Family::String(|form| form.reads_source() || form.reaches_destination()), Origin::Mandatory),
    row(Bug::StringCompareOperandsSwapped, "SCAS, CMPS", Family::String(|form| form.compares()), Origin::Mandatory),
    row(Bug::MoveChecksDestinationFirst, "MOVS", Family::String(|form| form.operation == crate::string::StringOperation::Move), Origin::Mandatory),
    row(Bug::StringSourceSegmentIgnored, "LODS, MOVS, CMPS", Family::String(|form| form.reads_source()), Origin::Mandatory),
    row(Bug::RepeatCountZeroRuns, "repeated string forms", Family::String(|form| form.is_repeated()), Origin::Mandatory),
    row(Bug::RepeatFlagsWrittenEachIteration, "REPE and REPNE SCAS and CMPS", Family::String(|form| form.is_repeated() && form.compares()), Origin::Mandatory),
    row(Bug::CompareReadsSourceFirst, "CMPS", Family::String(|form| form.operation == crate::string::StringOperation::Compare), Origin::Mandatory),
    row(Bug::PushFlagsOmitsInterrupt, "PUSHF, SYSCALL", Family::Forms(&[Instruction::Pushf, Instruction::Pushfq, Instruction::Syscall]), Origin::Mandatory),
    row(Bug::PopFlagsSixteenReachesUpper, "POPF", Family::Forms(&[Instruction::Popf]), Origin::Mandatory),
    row(Bug::PopFlagsKeepsAlignmentCheck, "POPF", Family::Forms(&[Instruction::Popf, Instruction::Popfq]), Origin::Mandatory),
    row(Bug::EnterNestingUnreduced, "ENTER", Family::Forms(&[Instruction::EnterImmwImmb]), Origin::Mandatory),
    row(Bug::EnterProbeSkipped, "ENTER", Family::Forms(&[Instruction::EnterImmwImmb]), Origin::Mandatory),
    row(Bug::MmxConvertIgnoresPendingException, "CVTPI2PS to CVTTPD2PI, MASKMOVQ", Family::Forms(MMX_CONVERTS), Origin::Mandatory),
    row(Bug::MmxConvertMisalignedNoFault, "CVTPD2PI, CVTTPD2PI", Family::Forms(&[Instruction::Cvtpd2piMmxqMempd, Instruction::Cvttpd2piMmxqMempd]), Origin::Mandatory),
    row(Bug::ConvertToSinglesZeroesUpperHalf, "CVTPI2PS", Family::Forms(&[Instruction::Cvtpi2psXmmqMmxq, Instruction::Cvtpi2psXmmqMemq]), Origin::Mandatory),
    row(Bug::ConvertFromMemoryEntersMmx, "CVTPI2PS, CVTPI2PD", Family::Forms(&[Instruction::Cvtpi2psXmmqMemq, Instruction::Cvtpi2pdXmmpdMemq]), Origin::Mandatory),
    row(Bug::MaskedQuadwordWritesEveryByte, "MASKMOVQ", Family::Forms(&[Instruction::MaskmovqMmxqMmxq]), Origin::Mandatory),
    row(Bug::MmxConvertEntersMmxLast, "CVTPI2PS to CVTTPD2PI, MASKMOVQ", Family::Forms(MMX_CONVERTS), Origin::Mandatory),
    row(Bug::X87NanSmallerSignificandWins, "FADD to FDIVR", Family::Forms(X87_ARITHMETIC_SAMPLE), Origin::Mandatory),
    row(Bug::X87DenormalOperandIgnored, "FADD to FDIVR", Family::Forms(X87_ARITHMETIC_SAMPLE), Origin::Mandatory),
    row(Bug::X87ReverseIgnored, "FSUBR, FDIVR", Family::Forms(&[Instruction::FsubrSt0X87, Instruction::FdivrX87St0, Instruction::FsubrSt0Memmem32real, Instruction::FidivrSt0Memmem16int]), Origin::Mandatory),
    row(Bug::X87ConstantsTruncated, "FLDPI to FLDLN2", Family::Forms(&[Instruction::Fldpi, Instruction::Fldl2e, Instruction::Fldl2t, Instruction::Fldlg2, Instruction::Fldln2]), Origin::Mandatory),
    row(Bug::X87PrecisionControlIgnored, "FADD to FDIVR, FSQRT", Family::Forms(X87_ARITHMETIC_SAMPLE), Origin::Mandatory),
    row(Bug::X87TinyBeforeRounding, "FADD to FDIVR, FST", Family::Forms(X87_ARITHMETIC_SAMPLE), Origin::Mandatory),
    row(Bug::X87RemainderNeverPartial, "FPREM, FPREM1", Family::Forms(&[Instruction::Fprem, Instruction::Fprem1]), Origin::Mandatory),
    row(Bug::X87AreaMisalignedNoFault, "FXSAVE, FXRSTOR", Family::Forms(X87_EXTENDED_AREAS), Origin::Mandatory),
    row(Bug::X87RestoreTakesReservedMxcsr, "FXRSTOR", Family::Forms(&[Instruction::FxrstorMemmfpxenv, Instruction::Fxrstor64Memmfpxenv]), Origin::Mandatory),
    row(Bug::X87EnvironmentSkipsSelectors, "FNSTENV, FNSAVE", Family::Forms(&[Instruction::FnstenvMemmem14, Instruction::FnsaveMemmem94]), Origin::Mandatory),
    row(Bug::X87EnvironmentAlignmentUnchecked, "FNSTENV to FRSTOR", Family::Forms(X87_ENVIRONMENTS), Origin::Mandatory),
    row(Bug::X87AreaFaultsAtFirstByte, "FNSTENV to FRSTOR, FXSAVE, FXRSTOR", Family::Forms(X87_ENVIRONMENTS), Origin::Mandatory),
    row(Bug::DescriptorTableIndicatorIgnored, "LAR, LSL, VERR, VERW", Family::Forms(DESCRIPTOR_QUERIES), Origin::Mandatory),
    row(Bug::DescriptorRequestedPrivilegeChecked, "LAR, LSL, VERR, VERW", Family::Forms(DESCRIPTOR_QUERIES), Origin::Mandatory),
    row(Bug::SegmentLimitInPages, "LSL", Family::Forms(&[Instruction::LslGprvGprz, Instruction::LslGprvMemw]), Origin::Mandatory),
    row(Bug::VerifyWriteTakesCode, "VERW", Family::Forms(&[Instruction::VerwGpr16, Instruction::VerwMemw]), Origin::Mandatory),
    row(Bug::SystemCallReturnsToItself, "SYSCALL", Family::Forms(&[Instruction::Syscall]), Origin::Mandatory),
    row(Bug::ReciprocalTableNeighbor, "RCPPS, RCPSS", Family::Forms(&[Instruction::RcppsXmmpsXmmps, Instruction::RcppsXmmpsMemps, Instruction::RcpssXmmssXmmss, Instruction::RcpssXmmssMemss]), Origin::Mandatory),
    row(Bug::InterruptReturnIgnoresNestedTask, "IRET", Family::Forms(&[Instruction::Iret, Instruction::Iretd, Instruction::Iretq]), Origin::Mandatory),
    row(Bug::SegmentLoadKeepsBase, "MOV to FS and GS, POP FS, POP GS, LFS, LGS", Family::Forms(SEGMENT_LOADS), Origin::Mandatory),
    row(Bug::NullLoadKeepsBase, "MOV to FS and GS, POP FS, POP GS, LFS, LGS", Family::Forms(SEGMENT_LOADS), Origin::Mandatory),
    row(Bug::ReturnToInnerPrivilegeAllowed, "RET far, IRET", Family::Forms(&[Instruction::RetFar, Instruction::RetFarImmw, Instruction::Iret, Instruction::Iretd, Instruction::Iretq]), Origin::Mandatory),
    row(Bug::SelectorPopReadsWholeSlot, "POP FS, POP GS", Family::Forms(&[Instruction::PopFs, Instruction::PopGs]), Origin::Mandatory),
    row(Bug::CompatibilityKeepsWholeStackPointer, "CALL far, JMP far, RET far, IRET", Family::Forms(&[Instruction::CallFarMemp2, Instruction::JmpFarMemp2, Instruction::RetFar, Instruction::RetFarImmw, Instruction::Iretq]), Origin::Mandatory),
    row(Bug::StackSlotsReadInOrder, "RET far, IRET", Family::Forms(&[Instruction::RetFar, Instruction::RetFarImmw, Instruction::Iret, Instruction::Iretd, Instruction::Iretq]), Origin::Mandatory),
    row(Bug::WritePkruIgnoresEdx, "WRPKRU", Family::Forms(&[Instruction::Wrpkru]), Origin::Mandatory),
    row(Bug::HypervisorCallAnswersZero, "VMCALL", Family::Forms(&[Instruction::Vmcall]), Origin::Mandatory),
    row(Bug::CallFarChecksTargetFirst, "CALL far", Family::Forms(&[Instruction::CallFarMemp2]), Origin::Mandatory),
    row(Bug::FarTargetUnchecked, "CALL far, JMP far, RET far, IRET", Family::Forms(&[Instruction::CallFarMemp2, Instruction::JmpFarMemp2, Instruction::RetFar, Instruction::RetFarImmw, Instruction::Iretq]), Origin::Mandatory),
    row(Bug::RootParitySwapped, "RSQRTPS, RSQRTSS", Family::Forms(&[Instruction::RsqrtpsXmmpsXmmps, Instruction::RsqrtpsXmmpsMemps, Instruction::RsqrtssXmmssXmmss, Instruction::RsqrtssXmmssMemss]), Origin::Mandatory),
    row(Bug::VexKeepsZmmUpperBits, "VEX writes of a vector register", Family::Avx(writes_vector_register), Origin::VexSecondPass),
    row(Bug::ZeroUpperKeepsZmmUpperBits, "VZEROUPPER", Family::Avx(|form| form.operation == AvxOperation::ZeroUpper), Origin::VexSecondPass),
    row(
        Bug::ZeroAllClearsHighRegisters,
        "VZEROUPPER, VZEROALL",
        Family::Avx(|form| matches!(form.operation, AvxOperation::ZeroUpper | AvxOperation::ZeroAll)),
        Origin::VexSecondPass,
    ),
    row(Bug::DotSaturationWraps, "VPDPBUSDS, VPDPWSSDS", Family::Avx(|form| matches!(form.operation, AvxOperation::DotAccumulate { saturate: true, .. })), Origin::VexSecondPass),
    row(Bug::DotUnsignedBytesSigned, "VPDPBUSD, VPDPBUSDS", Family::Avx(|form| matches!(form.operation, AvxOperation::DotAccumulate { words: false, .. })), Origin::VexSecondPass),
    row(Bug::SupervisorSaveChecksOperandFirst, "XSAVES, XRSTORS", Family::Forms(SUPERVISOR_SAVES), Origin::VexSecondPass),
    row(Bug::DotProductsWrappedFirst, "VPDPBUSDS, VPDPWSSDS", Family::Avx(|form| matches!(form.operation, AvxOperation::DotAccumulate { saturate: true, .. })), Origin::VexSecondPass),
    row(Bug::EvexSaturationWraps, "VPADDS, VPSUBUS", Family::EvexInteger(|form| matches!(form.operation, EvexIntegerOperation::Lanes(LaneOperation::AddSaturating(..) | LaneOperation::SubtractSaturating(..)))), Origin::EvexFirstHalf),
    row(Bug::EvexAverageTruncates, "VPAVGB, VPAVGW", Family::EvexInteger(|form| matches!(form.operation, EvexIntegerOperation::Lanes(LaneOperation::Average(_)))), Origin::EvexFirstHalf),
    row(Bug::EvexMultiplyAddBytesWraps, "VPMADDUBSW", Family::EvexInteger(|form| form.operation == EvexIntegerOperation::Lanes(LaneOperation::MultiplyAddBytes)), Origin::EvexFirstHalf),
    row(Bug::EvexPackAlwaysSigned, "VPACKUSWB, VPACKUSDW", Family::EvexInteger(|form| matches!(form.operation, EvexIntegerOperation::Lanes(LaneOperation::Pack(_, Signedness::Unsigned)))), Origin::EvexFirstHalf),
    row(Bug::EvexIntegerUnpackSourcesSwapped, "VPUNPCKL, VPUNPCKH", Family::EvexInteger(|form| matches!(form.operation, EvexIntegerOperation::Lanes(LaneOperation::UnpackHigh(_) | LaneOperation::UnpackLow(_)))), Origin::EvexFirstHalf),
    row(Bug::EvexByteShiftCountModulo, "VPSLLDQ, VPSRLDQ", Family::EvexInteger(|form| matches!(form.operation, EvexIntegerOperation::Lanes(LaneOperation::ShiftBytesLeft | LaneOperation::ShiftBytesRight))), Origin::EvexFirstHalf),
    row(Bug::EvexByteWordCompareSignednessSwapped, "VPCMPB, VPCMPUW", Family::EvexInteger(|form| matches!(form.operation, EvexIntegerOperation::CompareIntoMask(ComparePredicate::Immediate(_))) && matches!(form.lane, Lane::Byte | Lane::Word)), Origin::EvexFirstHalf),
    row(Bug::EvexBroadcastDestinationLane, "VPACKSSDW, VPACKUSDW broadcasts", Family::EvexInteger(|form| form.place == EvexIntegerPlace::Memory && form.can_broadcast() && form.source_lane() != form.lane), Origin::EvexFirstHalf),
    row(Bug::EvexBroadcastTupleFirstLane, "VBROADCASTI32X2 to VBROADCASTF64X4", Family::EvexInteger(|form| matches!(form.operation, EvexIntegerOperation::BroadcastTuple(_))), Origin::EvexFirstHalf),
    row(Bug::EvexDuplicateOddAsEven, "VMOVSHDUP", Family::EvexInteger(|form| form.operation == EvexIntegerOperation::Lanes(LaneOperation::DuplicateOddSingles)), Origin::EvexFirstHalf),
    row(Bug::EvexWholeReadSuppressed, "VPACKSSWB, VPUNPCKLBW, VPSADBW in memory", Family::EvexInteger(|form| form.place == EvexIntegerPlace::Memory && !form.is_element_wise() && form.can_mask()), Origin::EvexFirstHalf),
    row(Bug::EvexFromMaskInverted, "VPMOVM2B to VPMOVM2Q", Family::EvexInteger(|form| form.operation == EvexIntegerOperation::FromMask), Origin::EvexFirstHalf),
    row(Bug::EvexToMaskLowestBit, "VPMOVB2M to VPMOVQ2M", Family::EvexInteger(|form| form.operation == EvexIntegerOperation::ToMask), Origin::EvexFirstHalf),
    row(Bug::EvexExtendSignedWithZeros, "VPMOVSX", Family::EvexInteger(|form| matches!(form.operation, EvexIntegerOperation::Extend { signedness: Signedness::Signed, .. })), Origin::EvexFirstHalf),
    row(Bug::EvexNarrowTruncates, "VPMOVSDB, VPMOVUSQW", Family::EvexInteger(|form| matches!(form.operation, EvexIntegerOperation::Narrow { saturation: Some(_), .. })), Origin::EvexFirstHalf),
    row(Bug::EvexBlockDifferencesInOrder, "VDBPSADBW", Family::EvexInteger(|form| form.operation == EvexIntegerOperation::DoubleBlockDifferences), Origin::EvexFirstHalf),
    row(Bug::EvexInsertLowestPart, "VINSERTI32X4 to VINSERTF64X4", Family::EvexInteger(|form| matches!(form.operation, EvexIntegerOperation::Insert { .. })), Origin::EvexFirstHalf),
    row(Bug::EvexExtractIndexUnreduced, "VEXTRACTI32X4 to VEXTRACTF64X4", Family::EvexInteger(|form| matches!(form.operation, EvexIntegerOperation::Extract { .. })), Origin::EvexFirstHalf),
    row(Bug::EvexHalvesMovedTheOtherWay, "VMOVHLPS, VMOVLHPS, VMOVHPS", Family::EvexInteger(|form| matches!(form.operation, EvexIntegerOperation::Lanes(LaneOperation::MoveHighToLow | LaneOperation::MoveLowToHigh))), Origin::EvexFirstHalf),
    row(Bug::EvexInsertLaneLowest, "VPINSRB to VPINSRQ", Family::EvexInteger(|form| form.operation == EvexIntegerOperation::InsertLane), Origin::EvexFirstHalf),
    row(Bug::EvexMoveLowKeepsOthers, "VMOVD, VMOVQ", Family::EvexInteger(|form| matches!(form.operation, EvexIntegerOperation::Lanes(LaneOperation::MoveLowZeroing(_)))), Origin::EvexFirstHalf),
    row(Bug::EvexAndNotComplementsSecond, "VANDNPS, VPANDND", Family::EvexInteger(|form| form.operation == EvexIntegerOperation::Lanes(LaneOperation::AndNot)), Origin::EvexFirstHalf),
    row(Bug::HalfDenormalsAreZeros, "AVX512-FP16", Family::Evex(EvexForm::reads_half), Origin::EvexRest),
    row(Bug::HalfFlushesToZero, "AVX512-FP16", Family::Evex(EvexForm::writes_half), Origin::EvexRest),
    row(Bug::HalfUnderflowInexactUnbounded, "AVX512-FP16", Family::Evex(EvexForm::writes_half), Origin::EvexRest),
    row(Bug::EvexExpandReadsFromTheTop, "VEXPAND, VPEXPAND", Family::Evex(|form| matches!(form.operation, EvexOperation::Expand { .. })), Origin::EvexRest),
    row(Bug::EvexCompressReachesWholeVector, "VCOMPRESS, VEXPAND with memory", Family::Evex(|form| form.place != OperandPlace::Register && matches!(form.operation, EvexOperation::Compress { .. } | EvexOperation::Expand { .. })), Origin::EvexRest),
    row(Bug::MantissaIntervalThreeInverted, "VGETMANT", Family::Evex(|form| is_special(form, SpecialOperation::GetMantissa)), Origin::EvexRest),
    row(Bug::ScaleTruncatesPower, "VSCALEF", Family::Evex(|form| is_special(form, SpecialOperation::Scale)), Origin::EvexRest),
    row(Bug::ClassNegativeFiniteIncludesZero, "VFPCLASS", Family::Evex(|form| is_special(form, SpecialOperation::Class)), Origin::EvexRest),
    row(Bug::RoundScaleSuppressionIgnored, "VRNDSCALE, VREDUCE", Family::Evex(|form| is_special(form, SpecialOperation::RoundScale) || is_special(form, SpecialOperation::Reduce)), Origin::EvexRest),
    row(Bug::DotBfloat16LowerPairFirst, "VDPBF16PS", Family::Evex(|form| form.operation == EvexOperation::DotBfloat16), Origin::EvexRest),
    row(Bug::DotBfloat16SumNanFirst, "VDPBF16PS", Family::Evex(|form| form.operation == EvexOperation::DotBfloat16), Origin::EvexRest),
    row(Bug::TransactionTestClearsZero, "XTEST", Family::Forms(&[Instruction::Xtest]), Origin::EvexRest),
    row(Bug::TransactionEndCompletes, "XEND", Family::Forms(&[Instruction::Xend]), Origin::EvexRest),
    row(Bug::MoveWordKeepsUpperBits, "VMOVSH, VMOVW", Family::Evex(|form| form.operation == EvexOperation::MoveScalar && !form.reads_first_source()), Origin::EvexRest),
    row(Bug::FixupOneIgnoresSign, "VFIXUPIMM", Family::Evex(|form| is_special(form, SpecialOperation::Fixup)), Origin::EvexRest),
    row(Bug::ComplexConjugateSignsSwapped, "VFMULCPH to VFCMADDCSH", Family::Evex(|form| matches!(form.operation, EvexOperation::ComplexMultiply { .. })), Origin::EvexRest),
    row(Bug::ComplexHonorsMasks, "VFMULCPH to VFCMADDCSH", Family::Evex(|form| matches!(form.operation, EvexOperation::ComplexMultiply { .. })), Origin::EvexRest),
    row(Bug::HalfConversionImmediateIgnored, "VCVTPS2PH", Family::Evex(|form| matches!(form.operation, EvexOperation::Convert(_)) && form.has_immediate()), Origin::EvexRest),
    row(Bug::GatherScaleIgnored, "EVEX gathers and scatters", Family::Evex(|form| matches!(form.operation, EvexOperation::Gather { .. } | EvexOperation::Scatter { .. })), Origin::EvexRest),
    row(Bug::GatherFaultClearsMask, "EVEX gathers", Family::Evex(|form| matches!(form.operation, EvexOperation::Gather { .. })), Origin::EvexRest),
    row(Bug::ScatterMaskKept, "EVEX scatters", Family::Evex(|form| matches!(form.operation, EvexOperation::Scatter { .. })), Origin::EvexRest),
    row(Bug::RangeSignFromSelected, "VRANGE", Family::Evex(|form| is_special(form, SpecialOperation::Range)), Origin::EvexRest),
    row(Bug::EvexUnsignedIndefiniteSigned, "EVEX conversions to unsigned integers", Family::Evex(|form| matches!(form.operation, EvexOperation::Convert(Conversion { to: Number::Unsigned(_), .. }))), Origin::EvexRest),
    row(Bug::HalfToSingleReportsDenormal, "VCVTPH2PS", Family::Evex(|form| !form.reports_denormal()), Origin::EvexRest),
    row(Bug::IntegerToHalfOverflowExact, "VCVTW2PH to VCVTUQQ2PH", Family::Evex(|form| matches!(form.operation, EvexOperation::Convert(Conversion { from: Number::Signed(_) | Number::Unsigned(_), to: Number::Float(Precision::Half), .. }))), Origin::EvexRest),
    row(Bug::TruncatingStoreRoundsByControl, "FISTTP", Family::Forms(FISTTP), Origin::OptionalRemaining),
    row(Bug::MmxSsseThreeMemoryReadsSixteenBytes, "SSSE3 MMX forms in memory", Family::Forms(SSSE3_MMX_MEMORY), Origin::OptionalRemaining),
    row(Bug::ExtractIndexUnmasked, "EVEX VPEXTRB to VPEXTRQ, VEXTRACTPS", Family::OptionalForms(|form| matches!(form.operation, OptionalOperation::ExtractElement { bytes: 2.. }) && form.has_immediate()), Origin::OptionalRemaining),
    row(Bug::QuadwordStoreHalvesSwapped, "EVEX VMOVHPS to VMOVLPD", Family::OptionalForms(|form| matches!(form.operation, OptionalOperation::StoreQuadword { .. })), Origin::OptionalRemaining),
    row(Bug::OptionalMaskIgnored, "EVEX VMOVSS, VCVTPS2PH to memory, VRCP14, VRSQRTPH", Family::OptionalForms(OptionalForm::can_mask), Origin::OptionalRemaining),
    row(Bug::ScalarMoveKeepsUpperBits, "EVEX VMOVSS, VMOVSD of registers", Family::OptionalForms(|form| matches!(form.operation, OptionalOperation::MoveScalar { .. }) && form.place == OperandPlace::Register), Origin::OptionalRemaining),
    row(Bug::HalfStoreExceptionsIgnored, "EVEX VCVTPS2PH to memory", Family::OptionalForms(|form| form.operation == OptionalOperation::ConvertToHalves), Origin::OptionalRemaining),
    row(Bug::VexReciprocalKeepsUpperHalf, "VEX VRCPPS, VRSQRTPS of XMM", Family::OptionalForms(|form| matches!(form.operation, OptionalOperation::VexReciprocal { .. }) && form.vector_bytes() == 16), Origin::OptionalRemaining),
    row(Bug::OptionalMaskedFaultNotSuppressed, "VRCP14 to VRSQRTSH in memory", Family::OptionalForms(|form| matches!(form.operation, OptionalOperation::Reciprocal { .. }) && form.place == OperandPlace::Memory), Origin::OptionalRemaining),
    row(Bug::ReciprocalDenormalsAreZeros, "VRCP14, VRCPPH", Family::OptionalForms(|form| matches!(form.operation, OptionalOperation::Reciprocal { root: false, .. })), Origin::OptionalRemaining),
    row(Bug::ReciprocalFourteenIndexFifteenBits, "VRCP14", Family::OptionalForms(|form| matches!(form.operation, OptionalOperation::Reciprocal { root: false, precision: Precision::Single | Precision::Double })), Origin::OptionalRemaining),
    row(Bug::RootReciprocalParitySwapped, "VRSQRT14, VRSQRTPH", Family::OptionalForms(|form| matches!(form.operation, OptionalOperation::Reciprocal { root: true, .. })), Origin::OptionalRemaining),
    row(Bug::BaseWriteCanonicalUnchecked, "WRFSBASE, WRGSBASE", Family::OptionalForms(|form| matches!(form.operation, OptionalOperation::WriteBase { .. })), Origin::OptionalRemaining),
    row(Bug::MonitorChecksRead, "UMONITOR", Family::OptionalForms(|form| form.operation == OptionalOperation::Monitor), Origin::OptionalRemaining),
    row(Bug::HalfStoreFaultClearsFlags, "EVEX VCVTPS2PH to memory", Family::OptionalForms(|form| form.operation == OptionalOperation::ConvertToHalves), Origin::OptionalRemaining),
    row(Bug::HalfStoreFaultAtFirstByte, "EVEX VCVTPS2PH to memory", Family::OptionalForms(|form| form.operation == OptionalOperation::ConvertToHalves), Origin::OptionalRemaining),
    row(Bug::BlockStoreAlignmentUnchecked, "MOVDIR64B", Family::OptionalForms(|form| form.operation == OptionalOperation::DirectStoreBlock), Origin::OptionalRemaining),
    row(Bug::StringCompareAuxiliaryFromBitThree, "CMPS, SCAS", Family::String(|form| form.compares()), Origin::OptionalRemaining),
    row(Bug::SubtractAuxiliaryFromBitThree, "SUB, CMP, SBB", integer(|o| matches!(o, Operation::Subtract | Operation::Compare | Operation::SubtractWithBorrow)), Origin::OptionalRemaining),
    row(Bug::NegateAuxiliaryFromBitThree, "NEG", integer(|o| o == Operation::Negate), Origin::OptionalRemaining),
    row(Bug::VexReadDestinationKeepsZmmUpperBits, "VEX writes of a vector register", Family::Avx(writes_vector_register), Origin::OptionalRemaining),
    row(Bug::ControlReadIgnoresInUse, "XGETBV", Family::Forms(&[Instruction::Xgetbv]), Origin::StateAndXsave),
    row(Bug::ControlReadIgnoresMxcsr, "XGETBV", Family::Forms(&[Instruction::Xgetbv]), Origin::StateAndXsave),
    row(Bug::SaveHeaderDropsUnrequested, "XSAVE, XSAVEOPT", Family::Forms(STANDARD_SAVES), Origin::StateAndXsave),
    row(Bug::SaveMxcsrOnlyWithSse, "XSAVE, XSAVEOPT", Family::Forms(STANDARD_SAVES), Origin::StateAndXsave),
    row(Bug::OptimizedSavesInitial, "XSAVEOPT", Family::Forms(&[Instruction::XsaveoptMemmxsave, Instruction::Xsaveopt64Memmxsave]), Origin::StateAndXsave),
    row(Bug::CompactedIgnoresMxcsr, "XSAVEC", Family::Forms(&[Instruction::XsavecMemmxsave, Instruction::Xsavec64Memmxsave]), Origin::StateAndXsave),
    row(Bug::CompactedStandardOffsets, "XSAVEC", Family::Forms(&[Instruction::XsavecMemmxsave, Instruction::Xsavec64Memmxsave]), Origin::StateAndXsave),
    row(Bug::SaveRequestHalvesSwapped, "XSAVE family", Family::Forms(XSAVE_FAMILY), Origin::StateAndXsave),
    row(Bug::NarrowSaveWidePointers, "XSAVE, XSAVEC, XSAVEOPT", Family::Forms(&[Instruction::XsaveMemmxsave, Instruction::XsaveoptMemmxsave, Instruction::XsavecMemmxsave]), Origin::StateAndXsave),
    row(Bug::AreaAlignedToSixteen, "XSAVE family", Family::Forms(XSAVE_FAMILY), Origin::StateAndXsave),
    row(Bug::RestoreAcceptsUnenabledComponents, "XRSTOR", Family::Forms(&[Instruction::XrstorMemmxsave, Instruction::Xrstor64Memmxsave]), Origin::StateAndXsave),
    row(Bug::RestoreMxcsrOnlyWithSse, "XRSTOR", Family::Forms(&[Instruction::XrstorMemmxsave, Instruction::Xrstor64Memmxsave]), Origin::StateAndXsave),
    row(Bug::AreaFaultsAtLowestByte, "XSAVE family", Family::Forms(XSAVE_FAMILY), Origin::StateAndXsave),
    row(Bug::ZeroUpperTracksSseInitial, "VZEROUPPER", Family::Forms(&[Instruction::Vzeroupper]), Origin::StateAndXsave),
    row(Bug::RestoreReadsHeaderFromStart, "XRSTOR", Family::Forms(&[Instruction::XrstorMemmxsave, Instruction::Xrstor64Memmxsave]), Origin::StateAndXsave),
    row(Bug::RestoreTracksModified, "XRSTOR", Family::Forms(&[Instruction::XrstorMemmxsave, Instruction::Xrstor64Memmxsave]), Origin::StateAndXsave),
    row(Bug::TangentPushesZero, "FPTAN", Family::Forms(&[Instruction::Fptan]), Origin::X87AndRefused),
    row(Bug::SineCosineOrderSwapped, "FSINCOS", Family::Forms(&[Instruction::Fsincos]), Origin::X87AndRefused),
    row(Bug::TrigonometricRangeTo64, "FSIN, FCOS, FPTAN, FSINCOS", Family::Forms(&TRIGONOMETRIC), Origin::X87AndRefused),
    row(Bug::LogOfZeroNoDivideByZero, "FYL2X", Family::Forms(&[Instruction::Fyl2x]), Origin::X87AndRefused),
    row(Bug::ArcTangentZeroOverNegativeZero, "FPATAN", Family::Forms(&[Instruction::Fpatan]), Origin::X87AndRefused),
    row(Bug::TwoToXOutOfRangeExact, "F2XM1", Family::Forms(&[Instruction::F2xm1]), Origin::X87AndRefused),
    row(Bug::LogOnePlusBelowMinusOneInvalid, "FYL2XP1", Family::Forms(&[Instruction::Fyl2xp1]), Origin::X87AndRefused),
    row(Bug::TranscendentalDenormalIgnored, "the x87 transcendental forms", Family::Forms(&TRANSCENDENTAL), Origin::X87AndRefused),
    row(Bug::BinaryTranscendentalKeepsStack, "FPATAN, FYL2X, FYL2XP1", Family::Forms(&BINARY_TRANSCENDENTAL), Origin::X87AndRefused),
    row(Bug::UndecidedRegisterMisplaced, "FPATAN, FYL2X, FYL2XP1", Family::Forms(&BINARY_TRANSCENDENTAL), Origin::X87AndRefused),
    row(Bug::SystemEnterAdvances, "SYSENTER", Family::Forms(&[Instruction::Sysenter]), Origin::X87AndRefused),
    row(Bug::TimeStampCounterUnrestricted, "RDTSC, RDTSCP", Family::Forms(&[Instruction::Rdtsc, Instruction::Rdtscp]), Origin::X87AndRefused),
    row(Bug::WaitWithoutTimeStampControl, "UMWAIT, TPAUSE", Family::Forms(&[Instruction::UmwaitGpr32, Instruction::TpauseGpr32u32]), Origin::X87AndRefused),
    row(Bug::CountJumpTestsDoubleword, "JCXZ", Family::Forms(&[Instruction::JcxzRelbrb]), Origin::X87AndRefused),
    row(Bug::SixteenBitJumpTargetWhole, "JCXZ", Family::Forms(&[Instruction::JcxzRelbrb]), Origin::X87AndRefused),
    row(Bug::CompatibilityStepKeepsStackUpperHalf, "JCXZ", Family::Forms(&[Instruction::JcxzRelbrb]), Origin::X87AndRefused),
];

const FISTTP: &[Instruction] =
    &[Instruction::FisttpMemmem16intSt0, Instruction::FisttpMemmem32intSt0, Instruction::FisttpMemm64intSt0];
/// SSSE3's MMX forms with their source in memory.
pub const SSSE3_MMX_MEMORY: &[Instruction] = &[
    Instruction::PabsbMmxqMemq, Instruction::PabsdMmxqMemq, Instruction::PabswMmxqMemq, Instruction::PalignrMmxqMemqImmb,
    Instruction::PhadddMmxqMemq, Instruction::PhaddswMmxqMemq, Instruction::PhaddwMmxqMemq, Instruction::PhsubdMmxqMemq,
    Instruction::PhsubswMmxqMemq, Instruction::PhsubwMmxqMemq, Instruction::PmaddubswMmxqMemq, Instruction::PmulhrswMmxqMemq,
    Instruction::PshufbMmxqMemq, Instruction::PsignbMmxqMemq, Instruction::PsigndMmxqMemq, Instruction::PsignwMmxqMemq,
];

const STANDARD_SAVES: &[Instruction] = &[
    Instruction::XsaveMemmxsave,
    Instruction::Xsave64Memmxsave,
    Instruction::XsaveoptMemmxsave,
    Instruction::Xsaveopt64Memmxsave,
];

const XSAVE_FAMILY: &[Instruction] = &[
    Instruction::XsaveMemmxsave,
    Instruction::Xsave64Memmxsave,
    Instruction::XsaveoptMemmxsave,
    Instruction::Xsaveopt64Memmxsave,
    Instruction::XsavecMemmxsave,
    Instruction::Xsavec64Memmxsave,
    Instruction::XrstorMemmxsave,
    Instruction::Xrstor64Memmxsave,
];

const TRIGONOMETRIC: [Instruction; 4] = [Instruction::Fsin, Instruction::Fcos, Instruction::Fptan, Instruction::Fsincos];
const BINARY_TRANSCENDENTAL: [Instruction; 3] = [Instruction::Fpatan, Instruction::Fyl2x, Instruction::Fyl2xp1];
const TRANSCENDENTAL: [Instruction; 8] = [
    Instruction::F2xm1,
    Instruction::Fsin,
    Instruction::Fcos,
    Instruction::Fptan,
    Instruction::Fsincos,
    Instruction::Fpatan,
    Instruction::Fyl2x,
    Instruction::Fyl2xp1,
];

const SEGMENT_LOADS: &[Instruction] = &[
    Instruction::MovSegGpr16,
    Instruction::MovSegMemw,
    Instruction::PopFs,
    Instruction::PopGs,
    Instruction::LfsGprvMemp2,
    Instruction::LgsGprvMemp2,
];

const DESCRIPTOR_QUERIES: &[Instruction] = &[
    Instruction::LarGprvGprv,
    Instruction::LarGprvMemw,
    Instruction::LslGprvGprz,
    Instruction::LslGprvMemw,
    Instruction::VerrGpr16,
    Instruction::VerrMemw,
    Instruction::VerwGpr16,
    Instruction::VerwMemw,
];

const X87_EXTENDED_AREAS: &[Instruction] = &[
    Instruction::FxsaveMemmfpxenv,
    Instruction::Fxsave64Memmfpxenv,
    Instruction::FxrstorMemmfpxenv,
    Instruction::Fxrstor64Memmfpxenv,
];

const X87_ENVIRONMENTS: &[Instruction] = &[
    Instruction::FnstenvMemmem14,
    Instruction::FnstenvMemmem28,
    Instruction::FldenvMemmem14,
    Instruction::FldenvMemmem28,
    Instruction::FnsaveMemmem94,
    Instruction::FnsaveMemmem108,
    Instruction::FrstorMemmem94,
    Instruction::FrstorMemmem108,
    Instruction::FxsaveMemmfpxenv,
    Instruction::FxrstorMemmfpxenv,
];

const X87_ARITHMETIC_SAMPLE: &[Instruction] = &[
    Instruction::FaddSt0X87,
    Instruction::FsubX87St0,
    Instruction::FmulpX87St0,
    Instruction::FdivSt0Memm64real,
    Instruction::FdivrSt0Memmem32real,
    Instruction::FiaddSt0Memmem32int,
    Instruction::FstMemmem32realSt0,
    Instruction::FstpMemm64realSt0,
    Instruction::Fsqrt,
];

const MMX_CONVERTS: &[Instruction] = &[
    Instruction::Cvtpi2psXmmqMmxq,
    Instruction::Cvtpi2pdXmmpdMmxq,
    Instruction::Cvtps2piMmxqXmmq,
    Instruction::Cvtps2piMmxqMemq,
    Instruction::Cvttps2piMmxqXmmq,
    Instruction::Cvtpd2piMmxqXmmpd,
    Instruction::Cvtpd2piMmxqMempd,
    Instruction::MaskmovqMmxqMmxq,
];

/// Whether the AVX form writes a vector register: its first operand is one and it writes no flags, or it zeroes them.
fn writes_vector_register(form: &AvxForm) -> bool {
    use iced_x86::OpCodeOperandKind as Kind;
    let first = form.code.op_code().op_kind(0);
    (matches!(first, Kind::xmm_reg | Kind::ymm_reg) && !form.operation.writes_flags())
        || matches!(form.operation, AvxOperation::ZeroUpper | AvxOperation::ZeroAll)
}

const SUPERVISOR_SAVES: &[Instruction] =
    &[Instruction::XsavesMemmxsave, Instruction::Xsaves64Memmxsave, Instruction::XrstorsMemmxsave, Instruction::Xrstors64Memmxsave];

/// Whether the form is an EVEX form of the operation on a number's parts.
fn is_special(form: &EvexForm, wanted: SpecialOperation) -> bool {
    matches!(form.operation, EvexOperation::Special { operation, .. } if operation == wanted)
}

const RANDOM: &[Instruction] = &[Instruction::RdrandGprv, Instruction::RdseedGprv];

const X87_SAMPLE: &[Instruction] = &[Instruction::Fnop, Instruction::FxchSt0X87, Instruction::FldSt0Memmem80real, Instruction::Fchs];
const MMX_SAMPLE: &[Instruction] = &[
    Instruction::PaddbMmxqMmxq,
    Instruction::PsllqMmxqImmb,
    Instruction::MovdMmxqGpr32,
    Instruction::MovqMemqMmxq0f7f,
    Instruction::PextrwGpr32MmxqImmb,
];

const PRIVILEGED_OPERAND_FORMS: &[Instruction] = &[
    Instruction::LgdtMems64,
    Instruction::LldtGpr16,
    Instruction::MovCrCrGpr64,
    Instruction::InAlDx,
    Instruction::OutImmbOeax,
    Instruction::Insb,
    Instruction::SldtMemw,
];
const HINT_NO_OPERATIONS: &[Instruction] = &[
    Instruction::NopMemv0f18r4,
    Instruction::NopMemvGprv0f19,
    Instruction::NopGprvMemv0f1a,
];
const FLUSHES: &[Instruction] =
    &[Instruction::ClflushMemmprefetch, Instruction::ClflushoptMemmprefetch, Instruction::ClwbMemmprefetch];

const ANDN: &[Instruction] = &[
    Instruction::AndnGpr32dGpr32dGpr32d,
    Instruction::AndnGpr32dGpr32dMemd,
    Instruction::AndnGpr64qGpr64qGpr64q,
    Instruction::AndnGpr64qGpr64qMemq,
];
const BEXTR: &[Instruction] = &[
    Instruction::BextrGpr32dGpr32dGpr32d,
    Instruction::BextrGpr32dMemdGpr32d,
    Instruction::BextrGpr64qGpr64qGpr64q,
    Instruction::BextrGpr64qMemqGpr64q,
];
const BLSR: &[Instruction] = &[
    Instruction::BlsrGpr32dGpr32d,
    Instruction::BlsrGpr32dMemd,
    Instruction::BlsrGpr64qGpr64q,
    Instruction::BlsrGpr64qMemq,
];
const BZHI: &[Instruction] = &[
    Instruction::BzhiGpr32dGpr32dGpr32d,
    Instruction::BzhiGpr32dMemdGpr32d,
    Instruction::BzhiGpr64qGpr64qGpr64q,
    Instruction::BzhiGpr64qMemqGpr64q,
];
const LZCNT: &[Instruction] = &[Instruction::LzcntGprvGprv, Instruction::LzcntGprvMemv];
const PEXT: &[Instruction] = &[
    Instruction::PextGpr32dGpr32dGpr32d,
    Instruction::PextGpr32dGpr32dMemd,
    Instruction::PextGpr64qGpr64qGpr64q,
    Instruction::PextGpr64qGpr64qMemq,
];
const SHLX: &[Instruction] = &[
    Instruction::ShlxGpr32dGpr32dGpr32d,
    Instruction::ShlxGpr32dMemdGpr32d,
    Instruction::ShlxGpr64qGpr64qGpr64q,
    Instruction::ShlxGpr64qMemqGpr64q,
];
const ADOX: &[Instruction] =
    &[Instruction::AdoxGpr32dGpr32d, Instruction::AdoxGpr32dMemd, Instruction::AdoxGpr64qGpr64q, Instruction::AdoxGpr64qMemq];
const MULX: &[Instruction] = &[Instruction::MulxGpr32dGpr32dGpr32d, Instruction::MulxGpr64qGpr64qGpr64q];
const MOVBE: &[Instruction] = &[Instruction::MovbeGprvMemv];
const CMPXCHG16B: &[Instruction] = &[Instruction::Cmpxchg16bMemdq, Instruction::Cmpxchg16bLockMemdq];
const CMPXCHG8B: &[Instruction] = &[Instruction::Cmpxchg8bMemq, Instruction::Cmpxchg8bLockMemq];
const CMPXCHG_PAIR: &[Instruction] = &[
    Instruction::Cmpxchg8bMemq,
    Instruction::Cmpxchg8bLockMemq,
    Instruction::Cmpxchg16bMemdq,
    Instruction::Cmpxchg16bLockMemdq,
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

/// The opmask operation's result with the planted bug of its family.
pub(crate) fn mutate_opmask(operation: OpmaskOperation, bits: u32, first: u64, second: u64, immediate: u8, result: u64) -> u64 {
    let mask = crate::opmask::to_mask(bits);
    match (active(), operation) {
        (Some(Bug::OpmaskAddKeepsCarry), OpmaskOperation::Add) => (first & mask) + (second & mask),
        (Some(Bug::OpmaskShiftCountModulo), OpmaskOperation::ShiftLeft | OpmaskOperation::ShiftRight) => {
            operation.compute(bits, first, second, (u32::from(immediate) % bits) as u8)
        }
        (Some(Bug::OpmaskOrTestCarryOnAnyBit), OpmaskOperation::OrTest) => {
            (result & !1) | u64::from((first | second) & mask != 0)
        }
        (Some(Bug::OpmaskUnpackHalvesSwapped), OpmaskOperation::Unpack) => operation.compute(bits, second, first, immediate),
        _ => result,
    }
}

/// The EVEX integer form's result with the planted bug of its family.
pub(crate) fn mutate_evex(form: &EvexIntegerForm, destination: &Zmm, first: &Zmm, second: &Zmm, immediate: u8, result: Zmm) -> Zmm {
    let recompute = |operation: EvexIntegerOperation, first: &Zmm, second: &Zmm, immediate: u8| {
        EvexIntegerForm { operation, ..*form }.compute_unplanted(destination, first, second, immediate)
    };
    match (active(), form.operation) {
        (Some(Bug::TernaryLogicSourcesSwapped), EvexIntegerOperation::TernaryLogic) => recompute(form.operation, second, first, immediate),
        (Some(Bug::CompareLessOrEqualAsLess), EvexIntegerOperation::CompareIntoMask(ComparePredicate::Immediate(_))) if immediate & 7 == 2 => {
            recompute(form.operation, first, second, 1)
        }
        (Some(Bug::LaneShiftCountModulo), EvexIntegerOperation::ShiftByLane(_)) => {
            let bits = form.lane.to_bits();
            let counts: alloc::vec::Vec<u64> =
                to_lanes(form.lane, second, form.count_lanes()).iter().map(|count| count % u64::from(bits)).collect();
            recompute(form.operation, first, &from_lanes(form.lane, &counts), immediate)
        }
        (Some(Bug::RotateRightAsLeft), EvexIntegerOperation::Rotate { left: false, by_lane }) => {
            recompute(EvexIntegerOperation::Rotate { left: true, by_lane }, first, second, immediate)
        }
        (Some(Bug::EvexSaturationWraps), EvexIntegerOperation::Lanes(LaneOperation::AddSaturating(lane, _))) => {
            recompute(EvexIntegerOperation::Lanes(LaneOperation::Add(lane)), first, second, immediate)
        }
        (Some(Bug::EvexSaturationWraps), EvexIntegerOperation::Lanes(LaneOperation::SubtractSaturating(lane, _))) => {
            recompute(EvexIntegerOperation::Lanes(LaneOperation::Subtract(lane)), first, second, immediate)
        }
        (Some(Bug::EvexAverageTruncates), EvexIntegerOperation::Lanes(LaneOperation::Average(lane))) => {
            let count = form.count_lanes();
            let (a, b) = (to_lanes(lane, first, count), to_lanes(lane, second, count));
            from_lanes(lane, &a.iter().zip(&b).map(|(x, y)| (x + y) >> 1).collect::<alloc::vec::Vec<_>>())
        }
        (Some(Bug::EvexMultiplyAddBytesWraps), EvexIntegerOperation::Lanes(LaneOperation::MultiplyAddBytes)) => {
            let bytes = form.length.to_bits() as usize / 8;
            let (a, b) = (to_lanes(Lane::Byte, first, bytes), to_lanes(Lane::Byte, second, bytes));
            let sums: alloc::vec::Vec<u64> = (0..bytes / 2)
                .map(|pair| {
                    let product = |index: usize| a[index] as i64 * to_signed(b[index], u8::BITS);
                    (product(2 * pair) + product(2 * pair + 1)) as u64 & 0xFFFF
                })
                .collect();
            from_lanes(Lane::Word, &sums)
        }
        (Some(Bug::EvexPackAlwaysSigned), EvexIntegerOperation::Lanes(LaneOperation::Pack(lane, _))) => {
            recompute(EvexIntegerOperation::Lanes(LaneOperation::Pack(lane, Signedness::Signed)), first, second, immediate)
        }
        (Some(Bug::EvexIntegerUnpackSourcesSwapped), EvexIntegerOperation::Lanes(LaneOperation::UnpackHigh(_) | LaneOperation::UnpackLow(_))) => {
            recompute(form.operation, second, first, immediate)
        }
        (Some(Bug::EvexByteShiftCountModulo), EvexIntegerOperation::Lanes(LaneOperation::ShiftBytesLeft | LaneOperation::ShiftBytesRight)) => {
            recompute(form.operation, first, second, immediate % 16)
        }
        (Some(Bug::EvexByteWordCompareSignednessSwapped), EvexIntegerOperation::CompareIntoMask(ComparePredicate::Immediate(signedness)))
            if matches!(form.lane, Lane::Byte | Lane::Word) =>
        {
            let swapped = match signedness {
                Signedness::Signed => Signedness::Unsigned,
                Signedness::Unsigned => Signedness::Signed,
            };
            recompute(EvexIntegerOperation::CompareIntoMask(ComparePredicate::Immediate(swapped)), first, second, immediate)
        }
        (Some(Bug::EvexBroadcastTupleFirstLane), EvexIntegerOperation::BroadcastTuple(_)) => {
            recompute(EvexIntegerOperation::BroadcastTuple(1), first, second, immediate)
        }
        (Some(Bug::EvexDuplicateOddAsEven), EvexIntegerOperation::Lanes(LaneOperation::DuplicateOddSingles)) => {
            recompute(EvexIntegerOperation::Lanes(LaneOperation::DuplicateEvenSingles), first, second, immediate)
        }
        (Some(Bug::EvexAndNotComplementsSecond), EvexIntegerOperation::Lanes(LaneOperation::AndNot)) => {
            recompute(form.operation, second, first, immediate)
        }
        (Some(Bug::EvexFromMaskInverted), EvexIntegerOperation::FromMask) => recompute(form.operation, first, &[!second[0], 0, 0, 0], immediate),
        (Some(Bug::EvexToMaskLowestBit), EvexIntegerOperation::ToMask) => {
            let lanes = to_lanes(form.lane, second, form.count_lanes());
            [u128::from(lanes.iter().enumerate().fold(0u64, |all, (index, value)| all | (value & 1) << index)), 0, 0, 0]
        }
        (Some(Bug::EvexExtendSignedWithZeros), EvexIntegerOperation::Extend { from, .. }) => {
            recompute(EvexIntegerOperation::Extend { from, signedness: Signedness::Unsigned }, first, second, immediate)
        }
        (Some(Bug::EvexNarrowTruncates), EvexIntegerOperation::Narrow { from, .. }) => {
            recompute(EvexIntegerOperation::Narrow { from, saturation: None }, first, second, immediate)
        }
        (Some(Bug::EvexBlockDifferencesInOrder), EvexIntegerOperation::DoubleBlockDifferences) => recompute(form.operation, first, second, 0xE4),
        (Some(Bug::EvexInsertLowestPart), EvexIntegerOperation::Insert { .. }) => recompute(form.operation, first, second, 0),
        (Some(Bug::EvexInsertLaneLowest), EvexIntegerOperation::InsertLane) => recompute(form.operation, first, second, 0),
        (Some(Bug::EvexMoveLowKeepsOthers), EvexIntegerOperation::Lanes(LaneOperation::MoveLowZeroing(lane))) => {
            recompute(EvexIntegerOperation::Lanes(LaneOperation::MoveLow(lane)), destination, second, immediate)
        }
        (Some(Bug::EvexHalvesMovedTheOtherWay), EvexIntegerOperation::Lanes(LaneOperation::MoveHighToLow)) => {
            recompute(EvexIntegerOperation::Lanes(LaneOperation::MoveLowToHigh), first, second, immediate)
        }
        (Some(Bug::EvexHalvesMovedTheOtherWay), EvexIntegerOperation::Lanes(LaneOperation::MoveLowToHigh)) => {
            recompute(EvexIntegerOperation::Lanes(LaneOperation::MoveHighToLow), first, second, immediate)
        }
        (Some(Bug::EvexExtractIndexUnreduced), EvexIntegerOperation::Extract { quarters }) => {
            let index = usize::from(immediate) * quarters;
            core::array::from_fn(|quarter| if quarter < quarters && index + quarter < 4 { second[index + quarter] } else { 0 })
        }
        _ => result,
    }
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
        (Bug::SubtractAuxiliaryFromBitThree, Operation::Subtract | Operation::Compare | Operation::SubtractWithBorrow) => {
            let borrow = u64::from(carry && operation == Operation::SubtractWithBorrow);
            let difference = d.wrapping_sub(s).wrapping_sub(borrow) & mask;
            outcome.flags.set(Flags::AUXILIARY_CARRY, (d ^ s ^ difference) & 0x08 != 0);
        }
        (Bug::NegateAuxiliaryFromBitThree, Operation::Negate) => {
            outcome.flags.set(Flags::AUXILIARY_CARRY, (d ^ result) & 0x08 != 0);
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

/// An integer AVX operation's result with the planted bug of its family.
pub(crate) fn mutate_avx(form: &AvxForm, sources: &[Wide], immediate: u8, result: Wide) -> Wide {
    let Some(bug) = active() else {
        return result;
    };
    let first = sources.first().copied().unwrap_or_default();
    let second = sources.get(1).copied().unwrap_or(first);
    let recompute = |operation: AvxOperation, sources: &[Wide], immediate: u8| {
        let changed = AvxForm { operation, ..*form };
        crate::avx::compute_integer_unplanted(&changed, sources, immediate)
    };
    match (bug, form.operation) {
        (Bug::UpperHalfImmediateUnshifted, AvxOperation::Halves { operation, .. }) => {
            recompute(AvxOperation::Halves { operation, upper_shift: 0 }, sources, immediate)
        }
        (Bug::BroadcastSecondLane, AvxOperation::Broadcast(lane)) => {
            let shifted = [first[0] >> lane.to_bits(), first[1]];
            recompute(form.operation, &[shifted], immediate)
        }
        (Bug::ExtendUpperHalfFromLowLanes, AvxOperation::Extend(_)) => [result[0], result[0]],
        (Bug::ExtractLaneZero, AvxOperation::ExtractLane(_)) => recompute(form.operation, sources, 0),
        (Bug::VariableShiftCountWrapped, AvxOperation::ShiftEach { lane, .. }) => {
            let mask = crate::avx::fill(lane, u64::from(lane.to_bits() - 1));
            recompute(form.operation, &[first, [second[0] & mask, second[1] & mask]], immediate)
        }
        (Bug::BlendVariableLowBit, AvxOperation::BlendVariable(lane)) => {
            let third = sources.get(2).copied().unwrap_or_default();
            let moved = |value: u128| join(lane, split(lane, value).into_iter().map(|bits| bits << (lane.to_bits() - 1)));
            recompute(form.operation, &[first, second, [moved(third[0]), moved(third[1])]], immediate)
        }
        (Bug::PermuteHalvesNeverZero, AvxOperation::PermuteHalves) => recompute(form.operation, sources, immediate & 0x77),
        (Bug::PermuteDoublewordsTwoBits, AvxOperation::PermuteDoublewords) => {
            let mask = crate::avx::fill(Lane::Doubleword, 3);
            recompute(form.operation, &[[first[0] & mask, first[1] & mask], second], immediate)
        }
        (Bug::CarrylessSecondFromBitOne, AvxOperation::CarrylessMultiply) => {
            recompute(form.operation, sources, (immediate & 1) | ((immediate >> 1 & 1) << 4))
        }
        (Bug::SignsLowerHalfOnly, AvxOperation::Signs(_)) => recompute(form.operation, &[[first[0], 0]], immediate),
        _ => result,
    }
}
