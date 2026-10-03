//! `%h`/`%x`/`%o`/`%d` text for partially unknown values (IEEE 1800-2009
//! 21.2.1.4): a digit (or a whole `%d` value) whose bits are all x/z prints
//! lowercase, a partially unknown one prints uppercase `X`, or `Z` when it has
//! no x bit. `%b` stays per-bit lowercase. Every expected line is derived by
//! hand from the digit boundaries of the fixture value, not from the runtime.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

const SUITE: &str = "unknown_digits";

fn rep(c: char, n: usize) -> String {
    c.to_string().repeat(n)
}

fn line(tag: &str, h: &str, o: &str, d: &str, b: &str) -> String {
    format!("{tag} {h}|{o}|{d}|{b}\n")
}

#[test]
fn partial_and_whole_unknown_digits_print_by_lrm_case_rule() {
    let mut e = String::new();
    // Width 1: one single-bit digit in every radix.
    e += &line("w1.x", "x", "x", "x", "x");
    e += &line("w1.z", "z", "z", "z", "z");
    e += &line("w1.1", "1", "1", "1", "1");
    // Width 4: one hex digit; octal is a one-bit top digit over three bits.
    e += &line("w4.xxxx", "x", "xx", "x", "xxxx");
    e += &line("w4.zzzz", "z", "zz", "z", "zzzz");
    e += &line("w4.0x00", "X", "0X", "X", "0x00");
    e += &line("w4.0z00", "Z", "0Z", "Z", "0z00");
    e += &line("w4.xz01", "X", "xZ", "X", "xz01");
    e += &line("w4.zz01", "Z", "zZ", "Z", "zz01");
    e += &line("w4.xxzz", "X", "xX", "X", "xxzz");
    e += &line("w4.1010", "a", "12", "10", "1010");
    // Width 8: octal digits cover bits 7:6, 5:3 and 2:0.
    e += &line("w8.all_x", "xx", "xxx", "x", "xxxxxxxx");
    e += &line("w8.all_z", "zz", "zzz", "z", "zzzzzzzz");
    e += &line("w8.part_x", "0X", "0X0", "X", "0000x000");
    e += &line("w8.part_z", "Z0", "Z00", "Z", "0z000000");
    e += &line("w8.xxxxzzzz", "xz", "xXz", "X", "xxxxzzzz");
    e += &line("w8.zzzz0101", "z5", "zZ5", "Z", "zzzz0101");
    e += &line("w8.5a", "5a", "132", "90", "01011010");
    // Width 65: hex has 17 digits (top digit holds only bit 64), octal has 22
    // (top digit holds bits 64:63 and straddles the word boundary).
    e += &line(
        "w65.all_x",
        &rep('x', 17),
        &rep('x', 22),
        "x",
        &rep('x', 65),
    );
    e += &line(
        "w65.all_z",
        &rep('z', 17),
        &rep('z', 22),
        "z",
        &rep('z', 65),
    );
    e += &line(
        "w65.top_x",
        &format!("x{}", rep('0', 16)),
        &format!("X{}", rep('0', 21)),
        "X",
        &format!("x{}", rep('0', 64)),
    );
    e += &line(
        "w65.top1_low_z",
        &format!("1{}Z", rep('0', 15)),
        &format!("2{}Z", rep('0', 20)),
        "Z",
        &format!("1{}z", rep('0', 63)),
    );
    e += &line(
        "w65.straddle",
        &format!("zX{}", rep('0', 15)),
        &format!("X{}", rep('0', 21)),
        "X",
        &format!("zx{}", rep('0', 63)),
    );
    // Width 130: hex has 33 digits (top digit holds bits 129:128), octal has 44
    // (top digit holds only bit 129; digit 42 holds bits 128:126).
    e += &line(
        "w130.all_x",
        &rep('x', 33),
        &rep('x', 44),
        "x",
        &rep('x', 130),
    );
    e += &line(
        "w130.all_z",
        &rep('z', 33),
        &rep('z', 44),
        "z",
        &rep('z', 130),
    );
    e += &line(
        "w130.top_xx",
        &format!("x{}", rep('0', 32)),
        &format!("xX{}", rep('0', 42)),
        "X",
        &format!("xx{}", rep('0', 128)),
    );
    e += &line(
        "w130.top_z1",
        &format!("Z{}", rep('0', 32)),
        &format!("z4{}", rep('0', 42)),
        "Z",
        &format!("z1{}", rep('0', 128)),
    );
    // Bit 129 set, bit 70 x (hex digit 17, octal digit 23), bit 5 z (hex digit
    // 1, octal digit 1).
    e += &line(
        "w130.mixed",
        &format!("2{}X{}Z0", rep('0', 14), rep('0', 15)),
        &format!("1{}X{}Z0", rep('0', 19), rep('0', 21)),
        "X",
        &format!("1{}x{}z{}", rep('0', 58), rep('0', 64), rep('0', 5)),
    );
    e += "sformatf.w8 0X|0X|0X|0X0|X|0000x000\n";
    e += "sformatf.w4 Z|zZ|Z|zz01\n";
    e += &format!("sformatf.w130 Z{}|z4{}|Z\n", rep('0', 32), rep('0', 42));
    sim_cli::run_case_backend_parity(SUITE, "format", &e, &[], &[]);
}
