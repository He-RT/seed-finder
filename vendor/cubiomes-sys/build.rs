#[cfg(not(feature = "cc_build"))]
use std::process::Command;
use std::{collections::HashSet, env, path::PathBuf};

macro_rules! add_prefix {
    ($x:literal, $($y:literal),+) => {
        [$(
            concat!($x, $y),
        )+]
    };
}

const C_OBJECTS: [&str; 8] = add_prefix!(
    "cubiomes/",
    "noise.c",
    "biomes.c",
    "layers.c",
    "biomenoise.c",
    "generator.c",
    "finders.c",
    "util.c",
    "quadbase.c"
);

#[derive(Debug)]
struct IgnoreMacros(HashSet<String>);

impl bindgen::callbacks::ParseCallbacks for IgnoreMacros {
    fn will_parse_macro(&self, name: &str) -> bindgen::callbacks::MacroParsingBehavior {
        if self.0.contains(name) {
            bindgen::callbacks::MacroParsingBehavior::Ignore
        } else {
            bindgen::callbacks::MacroParsingBehavior::Default
        }
    }
}

#[derive(Debug)]
struct DeriveMacros(Vec<String>);

impl bindgen::callbacks::ParseCallbacks for DeriveMacros {
    fn add_derives(&self, _info: &bindgen::callbacks::DeriveInfo<'_>) -> Vec<String> {
        match _info.kind {
            bindgen::callbacks::TypeKind::Struct => Vec::new(),
            bindgen::callbacks::TypeKind::Enum => self.0.clone(),
            bindgen::callbacks::TypeKind::Union => Vec::new(),
        }
    }
}

fn main() {
    #[cfg(not(feature = "cc_build"))]
    build_with_make();

    #[cfg(feature = "cc_build")]
    build_with_cc();

    println!("cargo:rustc-link-search={}/", env::var("OUT_DIR").unwrap());
    println!("cargo:rustc-link-lib=static=cubiomes");

    let ignored_macros = IgnoreMacros(
        vec![
            "FP_INFINITE".into(),
            "FP_NAN".into(),
            "FP_NORMAL".into(),
            "FP_SUBNORMAL".into(),
            "FP_ZERO".into(),
        ]
        .into_iter()
        .collect(),
    );

    let target = env::var("TARGET").unwrap_or_default();
    let layout_tests = !target.contains("windows");

    let bindings = base_bindgen_builder("wrapper.h")
        .parse_callbacks(Box::new(ignored_macros))
        .newtype_enum(".*")
        .layout_tests(layout_tests)
        .generate()
        .expect("Unable to generate binding for cubiomes");

    // Generates rustified enums for use in a wrapper library
    let biome_enum_bindings = base_bindgen_builder("enum_wrapper.h")
        .parse_callbacks(Box::new(DeriveMacros(vec![
            "FromPrimitive".into(),
            "ToPrimitive".into(),
            "PartialOrd".into(),
            "Ord".into(),
        ])))
        .blocklist_function(".*") //Blocks all functions, as we are only intrested in the enums
        .allowlist_type(["BiomeID", "Dimension", "MCVersion", "StructureType"].join("|"))
        .rustified_non_exhaustive_enum(".*")
        .generate()
        .expect("Unable to generate rustified enums for cubiomes");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings");

    biome_enum_bindings
        .write_to_file(out_path.join("biome_enums.rs"))
        .expect("Couldn't write biome enums");
}

fn base_bindgen_builder(header: &str) -> bindgen::Builder {
    let mut builder = bindgen::Builder::default().header(header);

    if let Ok(target) = env::var("TARGET") {
        builder = builder.clang_arg(format!("--target={target}"));

        if target.contains("windows-msvc") {
            builder = builder.clang_arg("-fms-compatibility");
        }

        if target == "x86_64-pc-windows-gnu" {
            let mingw_prefix = env::var("MINGW_W64_PREFIX")
                .unwrap_or_else(|_| "/opt/homebrew/opt/mingw-w64".into());
            let sysroot = format!("{mingw_prefix}/toolchain-x86_64");
            let toolchain_include = format!("{sysroot}/x86_64-w64-mingw32/include");
            let gcc_include = format!("{sysroot}/lib/gcc/x86_64-w64-mingw32/15.2.0/include");

            builder = builder
                .detect_include_paths(false)
                .clang_arg(format!("--sysroot={sysroot}"))
                .clang_arg("-nostdinc")
                .clang_arg(format!("-I{toolchain_include}"))
                .clang_arg(format!("-I{gcc_include}"));
        }
    }

    builder
}

#[cfg(not(feature = "cc_build"))]
fn build_with_make() {
    if !Command::new("make")
        .arg("-C")
        .arg("cubiomes/")
        .status()
        .expect("Failed to build cubiomes")
        .success()
    {
        panic!("Make did not return 0")
    }

    if !Command::new("mv")
        .arg("cubiomes/libcubiomes.a")
        .arg(env::var("OUT_DIR").unwrap())
        .status()
        .expect("Failed to move libcubiomes")
        .success()
    {
        panic!("mv did not return 0")
    }
}

#[cfg(feature = "cc_build")]
fn build_with_cc() {
    cc::Build::new().files(C_OBJECTS).compile("cubiomes");
}
