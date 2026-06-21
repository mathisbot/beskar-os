use crate::{core::TestResult, ensure_eq};

pub fn test_abi() -> TestResult {
    simd_sse()?;
    simd_avx()?;

    Ok(())
}

fn simd_sse() -> TestResult {
    let a = [1.0f32, 2.0, 3.0, 4.0];
    let b = [5.0f32, 6.0, 7.0, 8.0];
    let mut result = [0.0f32; 4];

    unsafe {
        core::arch::asm!(
            "movups xmm0, [{a}]",
            "movups xmm1, [{b}]",
            "addps xmm0, xmm1",
            "movups [{out}], xmm0",
            a = in(reg) a.as_ptr(),
            b = in(reg) b.as_ptr(),
            out = in(reg) result.as_mut_ptr(),
            options(nostack, preserves_flags),
        );
    }

    ensure_eq!(result, [6.0, 8.0, 10.0, 12.0], "SIMD SSE test failed");
    Ok(())
}

fn simd_avx() -> TestResult {
    let a = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let b = [9.0f32, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0];
    let mut result = [0.0f32; 8];

    unsafe {
        core::arch::asm!(
            "vmovups ymm0, [{a}]",
            "vmovups ymm1, [{b}]",
            "vaddps ymm0, ymm0, ymm1",
            "vmovups [{out}], ymm0",
            a = in(reg) a.as_ptr(),
            b = in(reg) b.as_ptr(),
            out = in(reg) result.as_mut_ptr(),
            options(nostack, preserves_flags),
        );
    }

    ensure_eq!(
        result,
        [10.0f32, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0],
        "SIMD AVX test failed"
    );

    Ok(())
}
