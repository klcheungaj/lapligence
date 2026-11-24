//! Seeded input generation for the bounded fuzz corpora (FIN-002).
//!
//! The one generator of every corpus: a ChaCha stream from proptest's
//! `TestRng`, seeded from the fixed [`SEED`], the corpus name and the case
//! number. A corpus therefore produces the same inputs on every host and in
//! every run, and a failing case is named by `(corpus, case)` alone. Each
//! group root and the library's unit tests declare this file once.
#![allow(dead_code)]

use proptest::prelude::RngCore;
use proptest::test_runner::{RngAlgorithm, TestRng};

/// The fixed seed all corpora derive their streams from.
const SEED: u64 = 0xf1a0_02b5_eed0_c0de;

/// The case numbers a corpus test runs: `0..checked_in`, the bounded count
/// that belongs to the suite, unless `LLG_FUZZ_CASES` (`<count>` or
/// `<first>..<end>`) names a longer local campaign. A difference such a
/// campaign finds is minimized into a checked-in fixture; the count in the
/// tree stays as it is.
pub(crate) fn cases(checked_in: u64) -> std::ops::Range<u64> {
    let Ok(text) = std::env::var("LLG_FUZZ_CASES") else {
        return 0..checked_in;
    };
    let number = |text: &str| -> u64 {
        text.parse()
            .unwrap_or_else(|_| panic!("LLG_FUZZ_CASES: `{text}` is not a case number"))
    };
    match text.split_once("..") {
        Some((first, end)) => number(first)..number(end),
        None => 0..number(&text),
    }
}

/// A deterministic stream of choices for one case of one corpus.
pub(crate) struct Fuzz(TestRng);

impl Fuzz {
    pub(crate) fn new(corpus: &str, case: u64) -> Self {
        // FNV-1a over the corpus name, then SplitMix64 to spread the state
        // over the 32 seed bytes.
        let mut state = SEED ^ case.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        for byte in corpus.bytes() {
            state = (state ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
        let mut seed = [0u8; 32];
        for chunk in seed.chunks_mut(8) {
            state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut word = state;
            word = (word ^ (word >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            word = (word ^ (word >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            chunk.copy_from_slice(&(word ^ (word >> 31)).to_le_bytes());
        }
        Self(TestRng::from_seed(RngAlgorithm::ChaCha, &seed))
    }

    pub(crate) fn word(&mut self) -> u64 {
        self.0.next_u64()
    }

    /// A value in `0..bound`; `bound` must not be zero.
    pub(crate) fn below(&mut self, bound: u64) -> u64 {
        // The modulo bias is irrelevant for input generation.
        self.word() % bound
    }

    /// A value in `low..=high`.
    pub(crate) fn range(&mut self, low: u64, high: u64) -> u64 {
        low + self.below(high - low + 1)
    }

    /// True in `numerator` of `denominator` cases.
    pub(crate) fn chance(&mut self, numerator: u64, denominator: u64) -> bool {
        self.below(denominator) < numerator
    }

    pub(crate) fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }

    pub(crate) fn index(&mut self, length: usize) -> usize {
        self.below(length as u64) as usize
    }

    pub(crate) fn bytes(&mut self, length: usize) -> Vec<u8> {
        let mut bytes = vec![0; length];
        self.0.fill_bytes(&mut bytes);
        bytes
    }

    /// One structural mutation of `input`: a flipped bit, a replaced byte, a
    /// removed, repeated or moved range, an inserted `token`, or a
    /// truncation. `tokens` are the spellings that matter to the format.
    pub(crate) fn mutate(&mut self, input: &[u8], tokens: &[&[u8]]) -> Vec<u8> {
        let mut output = input.to_vec();
        if output.is_empty() {
            return self.pick(tokens).to_vec();
        }
        let at = self.index(output.len());
        let span = 1 + self.index((output.len() - at).min(64));
        match self.below(8) {
            0 => output[at] ^= 1 << self.below(8),
            1 => output[at] = self.word() as u8,
            2 => {
                output.drain(at..at + span);
            }
            3 => {
                let copy = output[at..at + span].to_vec();
                let times = 1 + self.index(8);
                for _ in 0..times {
                    output.splice(at..at, copy.iter().copied());
                }
            }
            4 => {
                let moved: Vec<u8> = output.drain(at..at + span).collect();
                let to = self.index(output.len() + 1);
                output.splice(to..to, moved);
            }
            5 | 6 => {
                let token = self.pick(tokens).to_vec();
                output.splice(at..at, token);
            }
            _ => output.truncate(at),
        }
        output
    }
}
