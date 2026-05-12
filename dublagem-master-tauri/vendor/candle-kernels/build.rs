use cudaforge::{KernelBuilder, Result};
use std::env;
use std::path::PathBuf;

fn extra_compute_cap_gencode_args() -> Vec<String> {
    let primary_compute_cap = env::var("CUDA_COMPUTE_CAP").ok();

    env::var("CUDA_EXTRA_COMPUTE_CAPS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|compute_cap| !compute_cap.is_empty())
        .filter(|compute_cap| primary_compute_cap.as_deref() != Some(*compute_cap))
        .flat_map(|compute_cap| {
            let normalized = compute_cap
                .trim_start_matches("sm_")
                .trim_start_matches("compute_");
            [
                "-gencode".to_string(),
                format!("arch=compute_{normalized},code=sm_{normalized}"),
            ]
        })
        .collect()
}

fn main() -> Result<()> {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=src/compatibility.cuh");
    println!("cargo::rerun-if-changed=src/cuda_utils.cuh");
    println!("cargo::rerun-if-changed=src/binary_op_macros.cuh");
    println!("cargo::rerun-if-env-changed=CUDA_EXTRA_COMPUTE_CAPS");

    // Build for PTX
    let is_target_msvc = env::var("TARGET")
        .map(|target| target.contains("msvc"))
        .unwrap_or(false);

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let ptx_path = out_dir.join("ptx.rs");
    let mut ptx_builder = KernelBuilder::new()
        .source_dir("src") // Scan src/ for .cu files
        .exclude(&["moe_*.cu"]) // Exclude moe kernels for ptx build
        .arg("--expt-relaxed-constexpr")
        .arg("-std=c++17")
        .arg("-O3");

    if is_target_msvc {
        ptx_builder = ptx_builder.arg("-Xcompiler").arg("/Zc:preprocessor");
    }

    let bindings = ptx_builder.build_ptx()?;

    bindings.write(&ptx_path)?;

    let mut moe_builder = KernelBuilder::default()
        .source_files(vec![
            "src/moe/moe_gguf.cu",
            "src/moe/moe_wmma.cu",
            "src/moe/moe_wmma_gguf.cu",
        ])
        .arg("--expt-relaxed-constexpr")
        .arg("-std=c++17")
        .arg("-O3");

    for gencode_arg in extra_compute_cap_gencode_args() {
        moe_builder = moe_builder.arg(&gencode_arg);
    }

    // Disable bf16 WMMA kernels on GPUs older than sm_80 (Ampere).
    // bf16 WMMA fragments require compute capability >= 8.0.
    let compute_cap = cudaforge::detect_compute_cap()
        .map(|arch| arch.base())
        .unwrap_or(80);
    if compute_cap < 80 {
        moe_builder = moe_builder.arg("-DNO_BF16_KERNEL");
    }

    if is_target_msvc {
        moe_builder = moe_builder
            .arg("-D_USE_MATH_DEFINES")
            .arg("-Xcompiler")
            .arg("/Zc:preprocessor")
            .arg("-Xcompiler")
            .arg("/MD");
    } else {
        moe_builder = moe_builder.arg("-Xcompiler").arg("-fPIC");
    }

    moe_builder.build_lib(out_dir.join("libmoe.a"))?;
    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rustc-link-lib=moe");
    println!("cargo:rustc-link-lib=dylib=cudart");
    if !is_target_msvc {
        println!("cargo:rustc-link-lib=stdc++");
    }
    Ok(())
}
