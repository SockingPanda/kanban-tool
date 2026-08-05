/// Maximum bytes exposed by the future run-log reader.
///
/// The canonical contract deliberately has no tail query; readers always use
/// this bounded suffix when the operation is implemented.
pub const RUN_LOG_TAIL_BYTES: usize = 256 * 1024;
