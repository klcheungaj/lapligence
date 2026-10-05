//! Compile-time packed descriptor and compact limb-kernel selection.

/// Descriptor used by every translation unit in a generated model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ValueBackend {
    /// Three-plane descriptor, value ABI 4.
    #[default]
    Legacy,
    /// Inline/A-B descriptor, value ABI 5.
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
    /// Portable C; no GMP discovery, headers or linkage.
    #[default]
    Portable,
    /// GMP mpn; requires 64-bit nail-free limbs.
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
    /// Descriptor backend. Legacy is the default.
    pub backend: ValueBackend,
    /// Compact kernel implementation. Portable is the default.
    pub kernel: CompactKernel,
}

impl ValueConfig {
    /// Parse the driver's environment selectors, rejecting inconsistent choices.
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
            &selector("LLG_VALUE_BACKEND", "legacy")?,
            &selector("LLG_COMPACT_KERNELS", "portable")?,
        )
    }

    /// Parse exact user-facing selector names.
    pub fn parse(backend: &str, kernel: &str) -> Result<Self, String> {
        let backend = match backend {
            "legacy" => ValueBackend::Legacy,
            "compact" => ValueBackend::Compact,
            _ => {
                return Err(format!(
                    "invalid LLG_VALUE_BACKEND `{backend}`; use legacy or compact"
                ))
            }
        };
        let kernel = match kernel {
            "portable" => CompactKernel::Portable,
            "gmp" => CompactKernel::Gmp,
            _ => {
                return Err(format!(
                    "invalid LLG_COMPACT_KERNELS `{kernel}`; use portable or gmp"
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
        assert_eq!(
            ValueConfig::parse("legacy", "portable").unwrap(),
            ValueConfig::default()
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
