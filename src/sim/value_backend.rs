//! Compile-time packed descriptor and compact limb-kernel selection.
//!
//! Generated models always use the compact descriptor with GMP kernels (the
//! default [`ValueConfig`]); there is no user-facing selector and no fallback.
//! The legacy descriptor and the portable kernels are development references
//! for parity tests and benchmarks, selected only through
//! `LLG_DEV_VALUE_BACKEND` and `LLG_DEV_COMPACT_KERNELS` or explicitly by
//! library callers ([`ValueConfig::LEGACY`], [`ValueConfig::COMPACT_PORTABLE`]).

/// Descriptor used by every translation unit in a generated model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ValueBackend {
    /// Three-plane descriptor, value ABI 4. Development reference only.
    Legacy,
    /// Inline/A-B descriptor, value ABI 5.
    #[default]
    Compact,
}

impl ValueBackend {
    /// Packed descriptor ABI, independent of the coroutine process ABI.
    pub const fn abi(self) -> u32 {
        match self {
            Self::Legacy => 4,
            Self::Compact => 5,
        }
    }

    /// Literal C selector.
    pub const fn selector(self) -> u8 {
        match self {
            Self::Legacy => 0,
            Self::Compact => 1,
        }
    }
}

/// Wide arithmetic kernels within the compact descriptor backend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CompactKernel {
    /// Portable C; no GMP discovery, headers or linkage. Development
    /// reference only.
    Portable,
    /// GMP mpn; requires 64-bit nail-free limbs.
    #[default]
    Gmp,
}

impl CompactKernel {
    /// Literal C selector.
    pub const fn selector(self) -> u8 {
        match self {
            Self::Portable => 0,
            Self::Gmp => 1,
        }
    }
}

/// Selection shared by emission and model/runtime builds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ValueConfig {
    /// Descriptor backend. Compact is the default.
    pub backend: ValueBackend,
    /// Compact kernel implementation. GMP is the default.
    pub kernel: CompactKernel,
}

impl ValueConfig {
    /// The development legacy reference.
    pub const LEGACY: Self = Self {
        backend: ValueBackend::Legacy,
        kernel: CompactKernel::Portable,
    };
    /// The development compact reference without GMP.
    pub const COMPACT_PORTABLE: Self = Self {
        backend: ValueBackend::Compact,
        kernel: CompactKernel::Portable,
    };
    /// The production selection: compact values with GMP kernels.
    pub const COMPACT_GMP: Self = Self {
        backend: ValueBackend::Compact,
        kernel: CompactKernel::Gmp,
    };

    /// The driver's selection: [`Self::COMPACT_GMP`] unless the development
    /// selectors `LLG_DEV_VALUE_BACKEND`/`LLG_DEV_COMPACT_KERNELS` choose a
    /// reference, rejecting inconsistent choices.
    pub fn from_env() -> Result<Self, String> {
        let selector = |name, default: &str| {
            std::env::var_os(name).map_or_else(
                || Ok(default.to_owned()),
                |value| {
                    value
                        .into_string()
                        .map_err(|_| format!("invalid {name}: selector must be UTF-8"))
                },
            )
        };
        Self::parse(
            &selector("LLG_DEV_VALUE_BACKEND", "compact")?,
            &selector("LLG_DEV_COMPACT_KERNELS", "gmp")?,
        )
    }

    /// Parse exact development selector names.
    pub fn parse(backend: &str, kernel: &str) -> Result<Self, String> {
        let backend = match backend {
            "legacy" => ValueBackend::Legacy,
            "compact" => ValueBackend::Compact,
            _ => {
                return Err(format!(
                    "invalid LLG_DEV_VALUE_BACKEND `{backend}`; use legacy or compact"
                ))
            }
        };
        let kernel = match kernel {
            "portable" => CompactKernel::Portable,
            "gmp" => CompactKernel::Gmp,
            _ => {
                return Err(format!(
                    "invalid LLG_DEV_COMPACT_KERNELS `{kernel}`; use portable or gmp"
                ))
            }
        };
        let config = Self { backend, kernel };
        config.validate()?;
        Ok(config)
    }

    /// Validate the orthogonal choices before writing any build products.
    pub fn validate(self) -> Result<(), String> {
        if self.backend == ValueBackend::Legacy && self.kernel == CompactKernel::Gmp {
            return Err("GMP kernels require the compact value backend".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_are_exact_and_kernels_are_orthogonal() {
        assert_eq!(ValueConfig::default(), ValueConfig::COMPACT_GMP);
        assert_eq!(
            ValueConfig::parse("legacy", "portable").unwrap(),
            ValueConfig::LEGACY
        );
        assert_eq!(
            ValueConfig::parse("compact", "portable").unwrap(),
            ValueConfig::COMPACT_PORTABLE
        );
        assert_eq!(
            ValueConfig::parse("compact", "portable")
                .unwrap()
                .backend
                .abi(),
            5
        );
        assert_eq!(
            ValueConfig::parse("compact", "gmp").unwrap().kernel,
            CompactKernel::Gmp
        );
        for (backend, kernel) in [
            ("legacy", "gmp"),
            ("Legacy", "portable"),
            ("compact", "GMP"),
            ("1", "0"),
        ] {
            assert!(ValueConfig::parse(backend, kernel).is_err());
        }
    }
}
