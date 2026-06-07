use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=src/bindings.h");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    std::fs::write(out_path.join("bindings.rs"), bindings())
        .expect("couldn't write dispatch bindings");
}

fn bindings() -> &'static str {
    r#"
pub type UInt32 = ::std::os::raw::c_uint;
pub type FourCharCode = UInt32;
pub type CFIndex = ::std::os::raw::c_long;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct __CFString {
    _unused: [u8; 0],
}
pub type CFStringRef = *const __CFString;

pub type CMItemIndex = CFIndex;
pub type CMTimeValue = i64;
pub type CMTimeScale = i32;
pub type CMTimeEpoch = i64;
pub type CMTimeFlags = u32;
pub type CMVideoCodecType = FourCharCode;
pub type VTEncodeInfoFlags = UInt32;

#[repr(C, packed(4))]
#[derive(Debug, Copy, Clone)]
pub struct CMTime {
    pub value: CMTimeValue,
    pub timescale: CMTimeScale,
    pub flags: CMTimeFlags,
    pub epoch: CMTimeEpoch,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct CMSampleTimingInfo {
    pub duration: CMTime,
    pub presentationTimeStamp: CMTime,
    pub decodeTimeStamp: CMTime,
}

extern "C" {
    pub static kCMTimeInvalid: CMTime;
    pub static kCMSampleAttachmentKey_NotSync: CFStringRef;
    pub fn CMTimeMake(value: i64, timescale: i32) -> CMTime;
}
"#
}
