#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused)]

use objc::*;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub type OSType = FourCharCode;
pub type CVReturn = ::std::os::raw::c_int;

pub const kCVReturnSuccess: CVReturn = 0;
pub const kCVPixelFormatType_32BGRA: OSType = u32::from_be_bytes(*b"BGRA");
pub const kCVPixelFormatType_420YpCbCr8Planar: OSType = u32::from_be_bytes(*b"y420");
pub const kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange: OSType =
    u32::from_be_bytes(*b"420v");
pub const kCVPixelFormatType_420YpCbCr8BiPlanarFullRange: OSType = u32::from_be_bytes(*b"420f");
pub const kCMVideoCodecType_H264: CMVideoCodecType = u32::from_be_bytes(*b"avc1");
