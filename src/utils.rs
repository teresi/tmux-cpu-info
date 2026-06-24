use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::error::CpuBarError;

/// Current realtime
pub fn now_realtime() -> Result<(u64, u32), CpuBarError> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?;
    let sec = now.as_secs();
    let nsec = now.subsec_nanos();
    Ok((sec, nsec))
}
