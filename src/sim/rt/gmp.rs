//! Bundled GMP sources for compact GMP kernels.
//!
//! The compact kernels call five `mpn` functions. Rather than requiring an
//! installed GMP, llg carries the generic-C subset that implements them from
//! the `vendor/gmp` submodule (GMP 6.3.0), the tables generated for 64-bit
//! limbs (`gmp/generated`, see `scripts/gmp_tables.py`) and the CMake recipe
//! that builds them (`gmp/llg_gmp.cmake`). Generated projects receive these
//! files under `gmp/`; the runtime archive compiles them once per cache entry.

/// One bundled GMP file from the submodule, published under `gmp/`.
macro_rules! vendor_file {
    ($path:expr) => {
        (
            concat!("gmp/", $path),
            include_str!(concat!("../../../vendor/gmp/", $path)),
        )
    };
}

/// One `mpn/generic` source file of the bundled subset.
macro_rules! mpn_file {
    ($name:literal) => {
        vendor_file!(concat!("mpn/generic/", $name, ".c"))
    };
}

/// Every bundled file as (path relative to the generated project, contents).
/// The `mpn` names match `LLG_GMP_MPN_SOURCES` in `gmp/llg_gmp.cmake`.
pub fn bundled_gmp_sources() -> &'static [(&'static str, &'static str)] {
    &[
        ("gmp/llg_gmp.cmake", include_str!("gmp/llg_gmp.cmake")),
        (
            "gmp/generated/mp_bases.h",
            include_str!("gmp/generated/mp_bases.h"),
        ),
        (
            "gmp/generated/mp_bases.c",
            include_str!("gmp/generated/mp_bases.c"),
        ),
        (
            "gmp/generated/fac_table.h",
            include_str!("gmp/generated/fac_table.h"),
        ),
        (
            "gmp/generated/fib_table.h",
            include_str!("gmp/generated/fib_table.h"),
        ),
        (
            "gmp/generated/sieve_table.h",
            include_str!("gmp/generated/sieve_table.h"),
        ),
        vendor_file!("COPYING.LESSERv3"),
        vendor_file!("COPYINGv2"),
        vendor_file!("COPYINGv3"),
        vendor_file!("gmp-h.in"),
        vendor_file!("gmp-impl.h"),
        vendor_file!("longlong.h"),
        vendor_file!("assert.c"),
        vendor_file!("errno.c"),
        vendor_file!("memory.c"),
        vendor_file!("mp_clz_tab.c"),
        vendor_file!("mp_minv_tab.c"),
        vendor_file!("tal-reent.c"),
        vendor_file!("mpn/generic/gmp-mparam.h"),
        // Included by mul_fft.c.
        vendor_file!("mpn/generic/add_n_sub_n.c"),
        mpn_file!("add"),
        mpn_file!("add_1"),
        mpn_file!("add_n"),
        mpn_file!("addmul_1"),
        mpn_file!("bdiv_dbm1c"),
        mpn_file!("bdiv_q_1"),
        mpn_file!("cmp"),
        mpn_file!("com"),
        mpn_file!("compute_powtab"),
        mpn_file!("dcpi1_div_qr"),
        mpn_file!("dcpi1_divappr_q"),
        mpn_file!("dive_1"),
        mpn_file!("divrem_1"),
        mpn_file!("divrem_2"),
        mpn_file!("get_str"),
        mpn_file!("invertappr"),
        mpn_file!("lshift"),
        mpn_file!("lshiftc"),
        mpn_file!("mod_34lsub1"),
        mpn_file!("mu_div_qr"),
        mpn_file!("mul"),
        mpn_file!("mul_1"),
        mpn_file!("mul_basecase"),
        mpn_file!("mul_fft"),
        mpn_file!("mul_n"),
        mpn_file!("mulmod_bknp1"),
        mpn_file!("mulmod_bnm1"),
        mpn_file!("neg"),
        mpn_file!("nussbaumer_mul"),
        mpn_file!("pre_divrem_1"),
        mpn_file!("rshift"),
        mpn_file!("sbpi1_div_qr"),
        mpn_file!("sbpi1_divappr_q"),
        mpn_file!("sqr"),
        mpn_file!("sqr_basecase"),
        mpn_file!("sqrmod_bnm1"),
        mpn_file!("sub"),
        mpn_file!("sub_1"),
        mpn_file!("sub_n"),
        mpn_file!("submul_1"),
        mpn_file!("tdiv_qr"),
        mpn_file!("toom22_mul"),
        mpn_file!("toom2_sqr"),
        mpn_file!("toom32_mul"),
        mpn_file!("toom33_mul"),
        mpn_file!("toom3_sqr"),
        mpn_file!("toom42_mul"),
        mpn_file!("toom43_mul"),
        mpn_file!("toom44_mul"),
        mpn_file!("toom4_sqr"),
        mpn_file!("toom53_mul"),
        mpn_file!("toom63_mul"),
        mpn_file!("toom6_sqr"),
        mpn_file!("toom6h_mul"),
        mpn_file!("toom8_sqr"),
        mpn_file!("toom8h_mul"),
        mpn_file!("toom_couple_handling"),
        mpn_file!("toom_eval_dgr3_pm1"),
        mpn_file!("toom_eval_dgr3_pm2"),
        mpn_file!("toom_eval_pm1"),
        mpn_file!("toom_eval_pm2"),
        mpn_file!("toom_eval_pm2exp"),
        mpn_file!("toom_eval_pm2rexp"),
        mpn_file!("toom_interpolate_12pts"),
        mpn_file!("toom_interpolate_16pts"),
        mpn_file!("toom_interpolate_5pts"),
        mpn_file!("toom_interpolate_6pts"),
        mpn_file!("toom_interpolate_7pts"),
        mpn_file!("toom_interpolate_8pts"),
        mpn_file!("zero_p"),
    ]
}

/// Content identity of the bundle (FNV-1a over paths and contents), for
/// runtime cache keys and the value build identity.
pub fn bundled_gmp_identity() -> u64 {
    static IDENTITY: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *IDENTITY.get_or_init(|| {
        let mut hash = 0xcbf29ce484222325u64;
        for (path, content) in bundled_gmp_sources() {
            for byte in path.bytes().chain([0]).chain(content.bytes()).chain([0]) {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        hash
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipe_sources_are_bundled() {
        let recipe = include_str!("gmp/llg_gmp.cmake");
        let start = recipe
            .find("set(LLG_GMP_MPN_SOURCES")
            .expect("recipe lists mpn sources");
        let end = start + recipe[start..].find(')').expect("closed list");
        let names = recipe[start..end].split_whitespace().skip(1);
        let bundled = bundled_gmp_sources()
            .iter()
            .map(|(path, _)| *path)
            .collect::<std::collections::HashSet<_>>();
        let mut count = 0;
        for name in names {
            let path = format!("gmp/mpn/generic/{name}.c");
            assert!(bundled.contains(path.as_str()), "{path} is not bundled");
            count += 1;
        }
        let mpn = bundled
            .iter()
            .filter(|path| path.starts_with("gmp/mpn/generic/") && path.ends_with(".c"))
            .count();
        // add_n_sub_n.c is included by mul_fft.c rather than compiled.
        assert_eq!(mpn, count + 1, "bundled mpn sources match the recipe");
    }
}
