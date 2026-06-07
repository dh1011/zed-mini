use std::{
    env,
    path::PathBuf,
    process::{self, Command},
};

fn main() {
    generate_dispatch_bindings();
    compile_metal_shaders();
    generate_shader_bindings();
}

fn generate_dispatch_bindings() {
    println!("cargo:rustc-link-lib=framework=System");
    println!("cargo:rerun-if-changed=src/platform/mac/dispatch.h");

    let bindings = bindgen::Builder::default()
        .header("src/platform/mac/dispatch.h")
        .allowlist_var("_dispatch_main_q")
        .allowlist_function("dispatch_async_f")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .layout_tests(false)
        .generate()
        .expect("unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("dispatch_sys.rs"))
        .expect("couldn't write dispatch bindings");
}

const SHADER_HEADER_PATH: &str = "./src/platform/mac/shaders/shaders.h";

fn compile_metal_shaders() {
    let shader_path = "./src/platform/mac/shaders/shaders.metal";
    let air_output_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("shaders.air");
    let metallib_output_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("shaders.metallib");

    println!("cargo:rerun-if-changed={}", SHADER_HEADER_PATH);
    println!("cargo:rerun-if-changed={}", shader_path);

    let output = Command::new("xcrun")
        .args([
            "-sdk",
            "macosx",
            "metal",
            "-gline-tables-only",
            "-mmacosx-version-min=10.15.7",
            "-MO",
            "-c",
            shader_path,
            "-o",
        ])
        .arg(&air_output_path)
        .output()
        .unwrap();

    if !output.status.success() {
        eprintln!(
            "metal shader compilation failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        process::exit(1);
    }

    let output = Command::new("xcrun")
        .args(["-sdk", "macosx", "metallib"])
        .arg(air_output_path)
        .arg("-o")
        .arg(metallib_output_path)
        .output()
        .unwrap();

    if !output.status.success() {
        eprintln!(
            "metallib compilation failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        process::exit(1);
    }
}

fn generate_shader_bindings() {
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    std::fs::write(out_path.join("shaders.rs"), shader_bindings())
        .expect("couldn't write shader bindings");
}

fn shader_bindings() -> &'static str {
    r#"
pub type vector_float2 = u64;
pub type vector_uchar4 = u32;

pub const GPUIQuadInputIndex_GPUIQuadInputIndexVertices: GPUIQuadInputIndex = 0;
pub const GPUIQuadInputIndex_GPUIQuadInputIndexQuads: GPUIQuadInputIndex = 1;
pub const GPUIQuadInputIndex_GPUIQuadInputIndexUniforms: GPUIQuadInputIndex = 2;
pub type GPUIQuadInputIndex = ::std::os::raw::c_uint;

pub const GPUIShadowInputIndex_GPUIShadowInputIndexVertices: GPUIShadowInputIndex = 0;
pub const GPUIShadowInputIndex_GPUIShadowInputIndexShadows: GPUIShadowInputIndex = 1;
pub const GPUIShadowInputIndex_GPUIShadowInputIndexUniforms: GPUIShadowInputIndex = 2;
pub type GPUIShadowInputIndex = ::std::os::raw::c_uint;

pub const GPUISpriteVertexInputIndex_GPUISpriteVertexInputIndexVertices: GPUISpriteVertexInputIndex = 0;
pub const GPUISpriteVertexInputIndex_GPUISpriteVertexInputIndexSprites: GPUISpriteVertexInputIndex = 1;
pub const GPUISpriteVertexInputIndex_GPUISpriteVertexInputIndexViewportSize: GPUISpriteVertexInputIndex = 2;
pub const GPUISpriteVertexInputIndex_GPUISpriteVertexInputIndexAtlasSize: GPUISpriteVertexInputIndex = 3;
pub type GPUISpriteVertexInputIndex = ::std::os::raw::c_uint;

pub const GPUISpriteFragmentInputIndex_GPUISpriteFragmentInputIndexAtlas: GPUISpriteFragmentInputIndex = 0;
pub type GPUISpriteFragmentInputIndex = ::std::os::raw::c_uint;

pub const GPUIPathAtlasVertexInputIndex_GPUIPathAtlasVertexInputIndexVertices: GPUIPathAtlasVertexInputIndex = 0;
pub const GPUIPathAtlasVertexInputIndex_GPUIPathAtlasVertexInputIndexAtlasSize: GPUIPathAtlasVertexInputIndex = 1;
pub type GPUIPathAtlasVertexInputIndex = ::std::os::raw::c_uint;

pub const GPUIImageVertexInputIndex_GPUIImageVertexInputIndexVertices: GPUIImageVertexInputIndex = 0;
pub const GPUIImageVertexInputIndex_GPUIImageVertexInputIndexImages: GPUIImageVertexInputIndex = 1;
pub const GPUIImageVertexInputIndex_GPUIImageVertexInputIndexViewportSize: GPUIImageVertexInputIndex = 2;
pub const GPUIImageVertexInputIndex_GPUIImageVertexInputIndexAtlasSize: GPUIImageVertexInputIndex = 3;
pub type GPUIImageVertexInputIndex = ::std::os::raw::c_uint;

pub const GPUIImageFragmentInputIndex_GPUIImageFragmentInputIndexAtlas: GPUIImageFragmentInputIndex = 0;
pub type GPUIImageFragmentInputIndex = ::std::os::raw::c_uint;

pub const GPUISurfaceVertexInputIndex_GPUISurfaceVertexInputIndexVertices: GPUISurfaceVertexInputIndex = 0;
pub const GPUISurfaceVertexInputIndex_GPUISurfaceVertexInputIndexSurfaces: GPUISurfaceVertexInputIndex = 1;
pub const GPUISurfaceVertexInputIndex_GPUISurfaceVertexInputIndexViewportSize: GPUISurfaceVertexInputIndex = 2;
pub const GPUISurfaceVertexInputIndex_GPUISurfaceVertexInputIndexAtlasSize: GPUISurfaceVertexInputIndex = 3;
pub type GPUISurfaceVertexInputIndex = ::std::os::raw::c_uint;

pub const GPUISurfaceFragmentInputIndex_GPUISurfaceFragmentInputIndexYAtlas: GPUISurfaceFragmentInputIndex = 0;
pub const GPUISurfaceFragmentInputIndex_GPUISurfaceFragmentInputIndexCbCrAtlas: GPUISurfaceFragmentInputIndex = 1;
pub type GPUISurfaceFragmentInputIndex = ::std::os::raw::c_uint;

pub const GPUIUnderlineInputIndex_GPUIUnderlineInputIndexVertices: GPUIUnderlineInputIndex = 0;
pub const GPUIUnderlineInputIndex_GPUIUnderlineInputIndexUnderlines: GPUIUnderlineInputIndex = 1;
pub const GPUIUnderlineInputIndex_GPUIUnderlineInputIndexUniforms: GPUIUnderlineInputIndex = 2;
pub type GPUIUnderlineInputIndex = ::std::os::raw::c_uint;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct GPUIUniforms {
    pub viewport_size: vector_float2,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct GPUIQuad {
    pub origin: vector_float2,
    pub size: vector_float2,
    pub background_color: vector_uchar4,
    pub border_top: f32,
    pub border_right: f32,
    pub border_bottom: f32,
    pub border_left: f32,
    pub border_color: vector_uchar4,
    pub corner_radius: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct GPUIShadow {
    pub origin: vector_float2,
    pub size: vector_float2,
    pub corner_radius: f32,
    pub sigma: f32,
    pub color: vector_uchar4,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct GPUISprite {
    pub origin: vector_float2,
    pub target_size: vector_float2,
    pub source_size: vector_float2,
    pub atlas_origin: vector_float2,
    pub color: vector_uchar4,
    pub compute_winding: u8,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct GPUIPathVertex {
    pub xy_position: vector_float2,
    pub st_position: vector_float2,
    pub clip_rect_origin: vector_float2,
    pub clip_rect_size: vector_float2,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct GPUIImage {
    pub origin: vector_float2,
    pub target_size: vector_float2,
    pub source_size: vector_float2,
    pub atlas_origin: vector_float2,
    pub border_top: f32,
    pub border_right: f32,
    pub border_bottom: f32,
    pub border_left: f32,
    pub border_color: vector_uchar4,
    pub corner_radius: f32,
    pub grayscale: u8,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct GPUISurface {
    pub origin: vector_float2,
    pub target_size: vector_float2,
    pub source_size: vector_float2,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct GPUIUnderline {
    pub origin: vector_float2,
    pub size: vector_float2,
    pub thickness: f32,
    pub color: vector_uchar4,
    pub squiggly: u8,
}
"#
}
