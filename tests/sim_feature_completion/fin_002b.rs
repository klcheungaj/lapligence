//! FIN-002 steps 3c and 3d on generated models: adversarial input for the
//! run-time readers and random expressions against an independent
//! evaluator (`fin_002b/expressions.rs`).
//!
//! `fuzz_inputs.sv` sends one file and the plusargs of the run through
//! `$readmemh`, `$readmemb`, `$fscanf`, `$fgets`, `$fgetc`, `$fread`,
//! `$sscanf` and `$value$plusargs`. The model is built once per optimizer
//! mode and then executed once per case of the fixed corpus: well-formed
//! memory images with an independent expected content, and mutated or
//! random bytes for which the run must still end normally: exit status 0,
//! the last line of the transcript printed, ASCII output, and on stderr
//! nothing but catalogued messages. A generated-runtime sanitizer run of
//! this suite (`tests/readme.md`) covers the readers' memory safety.
//!
//! The corpora share `support/fuzz.rs`; the parsers of the tools themselves
//! are in the `adversarial_inputs` suite of the `general` binary.
use crate::fuzz::{self, Fuzz};
use crate::{sim_cli, sim_harness};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[path = "fin_002b/expressions.rs"]
mod expressions;

const SUITE: &str = "feature_completion/fin_002b";
const RUN_TIMEOUT: Duration = Duration::from_secs(60);

/// Spellings with a meaning in a memory image or a scanned line.
const TOKENS: &[&[u8]] = &[
    b"@",
    b"@f",
    b"@10",
    b"@ffffffffffffffffffff",
    b"@-1",
    b"@x",
    b"//",
    b"/*",
    b"*/",
    b"x",
    b"z",
    b"_",
    b"\n",
    b" ",
    b"\t",
    b"\r\n",
    b"0",
    b"1",
    b"f",
    b"ffff",
    b"fffff",
    b"g",
    b"-",
    b"+",
    b"\0",
    b"\xff",
    b"%",
    b"1e400",
    b"-2147483649",
    b"99999999999999999999999",
    b"0x",
    b"1_",
    b"__",
    b"'h",
    b"?",
    b"12 ff abc\n",
];

/// The model `llg` built below `directory` (its default output directory).
fn model(directory: &Path) -> PathBuf {
    let name = format!("sim{}", std::env::consts::EXE_SUFFIX);
    let path = directory.join("build/sim/tb/build/bin").join(name);
    assert!(path.is_file(), "no built model at {}", path.display());
    path
}

/// A well-formed image for the 16 words of `words`, with the content the
/// load leaves: `None` where no word is given (the element keeps x).
fn image(fuzz: &mut Fuzz) -> (Vec<u8>, Vec<Option<String>>) {
    let mut text = Vec::new();
    let mut words = vec![None; 16];
    let mut address = 0;
    for _ in 0..fuzz.below(24) {
        match fuzz.below(10) {
            0 => text.extend_from_slice(b"// a comment @3 ffff\n"),
            1 => text.extend_from_slice(b"/* a\n comment 12 */"),
            2 => {
                address = fuzz.index(16);
                text.extend_from_slice(format!("@{address:x}").as_bytes());
            }
            _ if address < 16 => {
                // Four digits, with an underscore between two of them in
                // some words; an x or z digit covers its four bits.
                let digits: Vec<u8> = (0..4)
                    .map(|_| *fuzz.pick(b"0123456789abcdefABCDEFxzXZ"))
                    .collect();
                for (position, digit) in digits.iter().enumerate() {
                    if position == 2 && fuzz.chance(1, 4) {
                        text.push(b'_');
                    }
                    text.push(*digit);
                }
                words[address] = Some(String::from_utf8_lossy(&digits).to_ascii_lowercase());
                address += 1;
            }
            _ => {}
        }
        text.extend_from_slice(fuzz.pick(&[&b" "[..], b"\n", b"\t", b"  \r\n"]));
    }
    (text, words)
}

fn run(executable: &Path, directory: &Path, arguments: &[String], label: &str) -> String {
    let mut command = Command::new(executable);
    command.current_dir(directory).args(arguments);
    let output = sim_harness::run_command(&mut command, RUN_TIMEOUT)
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{label}: {stderr}");
    for line in stderr.lines() {
        assert!(
            llg::core::report::parse(line).is_some(),
            "{label}: stderr line is not a catalogued message: {line}"
        );
    }
    assert!(output.stdout.is_ascii(), "{label}: output is not ASCII");
    let stdout = String::from_utf8(output.stdout).expect("ASCII is UTF-8");
    assert!(
        stdout.ends_with("done\n"),
        "{label}: the run stopped early:\n{stdout}\n{stderr}"
    );
    stdout
}

#[test]
fn runtime_readers_end_normally_on_adversarial_input() {
    for optimized in [true, false] {
        let (directory, built) =
            sim_cli::run_case_keeping_dir(SUITE, "fuzz_inputs", optimized, &[], &[]);
        assert_eq!(
            built.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let executable = model(directory.path());
        let input = directory.path().join("in.mem");
        for case in fuzz::cases(120) {
            let label = format!("inputs/readers case {case}, optimized={optimized}");
            let mut fuzz = Fuzz::new("inputs/readers", case);
            let (mut bytes, words) = image(&mut fuzz);
            // One case in three stays well formed and is checked exactly.
            let exact = fuzz.chance(1, 3);
            if !exact {
                if fuzz.chance(1, 5) {
                    let length = fuzz.index(300);
                    bytes = fuzz.bytes(length);
                }
                for _ in 0..fuzz.range(1, 3) {
                    bytes = fuzz.mutate(&bytes, TOKENS);
                }
            }
            std::fs::write(&input, &bytes).expect("write the input file");
            let decimal = fuzz.below(1 << 31);
            let hexadecimal = fuzz.word();
            let mut arguments = vec![format!("+dec={decimal}"), format!("+hex={hexadecimal:x}")];
            for _ in 0..fuzz.below(4) {
                let name = *fuzz.pick(&["text", "bin", "oct", "real", "fl", "", "dec", "hex"]);
                let value: String = (0..fuzz.below(40))
                    .map(|_| *fuzz.pick(b"0123456789abcdefxzXZ_-+. eE%snd") as char)
                    .collect();
                arguments.push(format!("+{name}={value}"));
            }
            let stdout = run(&executable, directory.path(), &arguments, &label);
            // The first match of a plusarg is the one that counts (21.6).
            assert!(
                stdout.contains(&format!("\ndec {decimal}\n")),
                "{label}:\n{stdout}"
            );
            assert!(
                stdout.contains(&format!("\nhex {hexadecimal:016x}\n")),
                "{label}:\n{stdout}"
            );
            if exact {
                for (address, word) in words.iter().enumerate() {
                    let expected = format!("h {address} {}\n", word.as_deref().unwrap_or("xxxx"));
                    assert!(
                        stdout.contains(&expected),
                        "{label}: expected `{}` from this image:\n{}\n{stdout}",
                        expected.trim_end(),
                        String::from_utf8_lossy(&bytes)
                    );
                }
            }
        }
    }
}

/// Minimized from `expressions/variables` case 11 and
/// `expressions/parameters` case 1: an unbased unsized literal below a
/// unary operator, a conditional, a shift or a power inside a size cast
/// was evaluated as one bit and then extended.
#[test]
fn unsized_literals_take_the_size_of_a_cast() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "fuzz_expr_cast_fill",
        include_str!("../fixtures/sim/feature_completion/fin_002b/fuzz_expr_cast_fill.out"),
        &[],
        &[],
    );
}
