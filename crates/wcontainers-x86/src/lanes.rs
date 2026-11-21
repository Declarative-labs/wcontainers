// How a VEX instruction's 128-bit lane runs as its legacy SSE form. The build script and the crate both include
// this file.

/// No legacy form runs a lane of it.
pub const LANE_NONE: u8 = 0;
/// The legacy form takes the operands in order.
pub const LANE_IN_ORDER: u8 = 1;
/// The legacy form leaves out the second operand (VEX.vvvv), which its destination starts as.
pub const LANE_FROM_FIRST_SOURCE: u8 = 2;
/// As `LANE_FROM_FIRST_SOURCE`, and the last operand is the one the legacy form reads from xmm0.
pub const LANE_FROM_FIRST_SOURCE_MASK_LAST: u8 = 3;
