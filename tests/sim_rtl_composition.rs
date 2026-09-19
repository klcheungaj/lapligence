//! G1-34 composed practical-RTL acceptance (`rtl_composition_gate` and
//! `rtl_no_silent_omissions`).
//!
//! Each fixture combines several Group 1 features instead of exercising them in
//! isolation: parameterized hierarchy with generate and a nested module instance
//! array, interfaces with shared modports carrying a packed structure, fixed
//! arrays/structures with copy and slices, zero-time subroutines, arithmetic and
//! expression sizing with wide values, resettable sequential logic, tri-state
//! and wired nets, declaration-initialized memory, and edition-scoped 2001/2009
//! variants. Designs run through `llg` with and without optimization on the
//! shared `support/sim_cli.rs` harness, which matches exact stdout and exact
//! stderr, so an unexpected lowering or migration diagnostic fails the test.
//!
//! Every expected trace is derived here from Rust integer arithmetic and the
//! LRM bit-width rules; no expected value is captured from the simulator.

#[path = "support/sim.rs"]
mod sim_harness;

#[path = "support/sim_cli.rs"]
mod sim_cli;

mod g1_34 {
    use super::sim_cli;

    const SUITE: &str = "feature_completion/g1_34";

    /// Parameterized, generated hierarchy (`g1_mac` inside a nested module
    /// instance array in `g1_bank`) plus a generated `always_ff` bank, a
    /// generate-if/case selection, signed multiplication and mixed
    /// signed/unsigned addition.
    #[test]
    fn rtl_composition_hierarchy_generate_pipeline() {
        let av = [1i64, -2, 3, -4];
        let bv = [5i64, -6, 7, -8];
        let yv: Vec<i64> = av.iter().zip(bv).map(|(a, b)| a * b + 1).collect();
        let total: i64 = yv.iter().sum();
        let add = 0x0fu32 + 0x33;
        let xor = 0x0fu32 ^ 0x33;
        let and = 0x0fu32 & 0x33;
        // IEEE 1800-2009 11.8.1: one unsigned operand makes the common type
        // unsigned, so -8'sd1 becomes 8'hFF = 255.
        let mixed = 0x80u32 + 0xFF;
        let expected = format!(
            "lw={lw}\n\
             yv={y0} {y1} {y2} {y3}\n\
             sum={y0} {y1} {y2} {y3} accum={total}\n\
             gen={gen} if={ifv} add={add} xor={xor} and={and}\n\
             mixed={mixed}\n",
            lw = 2,
            y0 = yv[0],
            y1 = yv[1],
            y2 = yv[2],
            y3 = yv[3],
            total = total,
            gen = 8,
            ifv = 0x0fu32 + 0x33,
        );
        sim_cli::run_case(SUITE, "hier_generate_pipeline", &expected, "", &[]);
    }

    /// One interface instance shared through complementary modports; the
    /// interface carries a packed structure and each side acts under a
    /// resettable `always_ff`. The consumer output register demonstrates
    /// nonblocking ordering (it observes the previous cycle's valid).
    #[test]
    fn rtl_composition_interface_modport_struct() {
        let tag = 0x2au16 + 1;
        let payload = 0xbeefu32 + 0x0101;
        let expected = format!(
            "reset wv=0 rv=0 rt=00 rp=0000 ov=0 of=000000\n\
             t1 wv=1 rv=0 rt=00 rp=0000 ov=0 of=000000\n\
             t2 wv=0 rv=1 rt={tag:02x} rp={payload:04x} ov=0 of=000000\n\
             t3 wv=0 rv=0 rt={tag:02x} rp={payload:04x} ov=1 of={tag:02x}{payload:04x}\n",
            tag = tag,
            payload = payload,
        );
        sim_cli::run_case(SUITE, "interface_modport_struct", &expected, "", &[]);
    }

    /// Fixed unpacked arrays (assignment patterns, whole-array copy, slice
    /// assignment), packed/unpacked structures with nested member selects, and
    /// zero-time subroutines (recursion, defaults, output/inout copy-out, ref
    /// formals, packed-structure arguments).
    #[test]
    fn rtl_composition_aggregate_functions() {
        let src = [0x10u8, 0x20, 0x30, 0x40];
        let mut part = [0u8; 4];
        part[1] = src[0];
        part[2] = src[1];
        part[3] = src[0];
        let sum: u16 = src.iter().map(|value| u16::from(*value)).sum();
        let expected = format!(
            "arr src=10203040 dst=10203040 part={p0:02x}{p1:02x}{p2:02x}{p3:02x}\n\
             sum={sum}\n\
             struct a=aa55 b=1122 idx=3 va=aa55 vb=1122\n\
             mix=15 p=1005 acc=2007 o=2007 sw=0510\n\
             defaults calls=1 y=41\n\
             defaults calls=1 y=5\n\
             combine s=15 c=107\n\
             combine s=11 c=112\n\
             ref r=16\n\
             fib=55\n",
            p0 = part[0],
            p1 = part[1],
            p2 = part[2],
            p3 = part[3],
        );
        sim_cli::run_case(SUITE, "aggregate_functions", &expected, "", &[]);
    }

    /// Mixed signed/unsigned arithmetic, 96-bit vectors, reductions, shifts,
    /// comparisons and set membership. The wide results are recomputed here
    /// with 96-bit masking and two's-complement sign extension.
    #[test]
    fn rtl_composition_arith_sizing_wide() {
        const MASK96: u128 = (1u128 << 96) - 1;
        let wide_a: u128 = 0x0123_4567_89AB_CDEF_FEDC_BA98;
        let wide_b: u128 = 0xFEDC_BA98_7654_3210_0123_4567;
        let wide_neg_bits: u128 = 0xF000_0000_0000_0000_0000_0001;
        let wide_sum = (wide_a + wide_b) & MASK96;
        let wide_shl = (wide_a << 13) & MASK96;
        // Interpret the low 96 bits as a signed value, then arithmetic-shift.
        let wide_neg = (wide_neg_bits as i128) - (1i128 << 96);
        let wide_sra = ((wide_neg >> 8) as u128) & MASK96;

        let add: i32 = 0x80 + 0xFF;
        let sub: i32 = 0x80 - 0xFF;
        let lt = u8::from(0x80u8 < 0xFFu8);
        let gt = u8::from(-1i8 > 0);
        let red_xor = u8::from(wide_a.count_ones() % 2 == 1);
        let red_and = u8::from(wide_a == MASK96);
        let red_or = u8::from(wide_a != 0);
        let wide_eq = u8::from(wide_a == wide_b);
        let wide_ne = u8::from(wide_sum != 0);

        let x = 5u8;
        let in1 = u8::from([1u8, 5, 9].contains(&x));
        let in2 = u8::from((6u8..=8).contains(&x));
        let in3 = u8::from([5u8].contains(&x) || (7u8..=9).contains(&x));

        let expected = format!(
            "mixed add={add} sub={sub} lt={lt} gt={gt}\n\
             wide sum={wide_sum:x} shl={wide_shl:x} sra={wide_sra:x}\n\
             red xor={red_xor} and={red_and} or={red_or} eq={wide_eq} ne={wide_ne}\n\
             inside {in1} {in2} {in3}\n",
        );
        sim_cli::run_case(SUITE, "arith_sizing_wide", &expected, "", &[]);
    }

    /// Scalar tri-state drive strength (strong beats weak, IEEE 1800-2009
    /// 10.3.4 permits strength on a scalar net), a multi-driver 4-bit tri bus
    /// (conflicting strong drivers resolve to x), wired-AND/wired-OR truth
    /// tables, and a resettable `always_ff` capture of the resolved bus.
    #[test]
    fn rtl_composition_tristate_wired_seq() {
        let expected = concat!(
            "z sbus=z bus2=z wand=0 wor=0\n",
            "a sbus=1\n",
            "ab sbus=1\n",
            "strong sbus=0\n",
            "b sbus=0\n",
            "conflict bus2=x\n",
            "cap=5 bus2=5\n",
            "wired1 wand=0 wor=1\n",
            "wired2 wand=1 wor=1\n",
            "wired3 wand=x wor=1\n",
            "wired4 wand=0 wor=0\n",
        );
        sim_cli::run_case(SUITE, "tristate_wired_seq", expected, "", &[]);
    }

    /// Declaration-initialized memory arrays, whole-array copy, memory slice
    /// assignment, a runtime index loop and a function that reads memory.
    #[test]
    fn rtl_composition_memory_init_index() {
        let rom: [u8; 8] = [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80];
        let ram = [0x00u8, rom[2], rom[3], 0x00];
        let paired = [
            rom[0] + rom[1],
            rom[2] + rom[3],
            rom[4] + rom[5],
            rom[6] + rom[7],
        ];
        let expected = format!(
            "rom0={rom0:02x} rom7={rom7:02x} copy3={copy3:02x}\n\
             ram={r0:02x} {r1:02x} {r2:02x} {r3:02x}\n\
             lookup={lookup:02x}\n\
             paired={p0:02x} {p1:02x} {p2:02x} {p3:02x}\n",
            rom0 = rom[0],
            rom7 = rom[7],
            copy3 = rom[3],
            r0 = ram[0],
            r1 = ram[1],
            r2 = ram[2],
            r3 = ram[3],
            lookup = rom[5],
            p0 = paired[0],
            p1 = paired[1],
            p2 = paired[2],
            p3 = paired[3],
        );
        sim_cli::run_case(SUITE, "memory_init_index", &expected, "", &[]);
    }

    fn edition_scope_expected() -> String {
        let combined: u8 = (0xF0 & 0x3C) | (0xF0 ^ 0x3C);
        let lanes = [
            combined,
            combined.wrapping_add(1),
            combined.wrapping_add(2),
            combined.wrapping_add(3),
        ];
        let acc: u16 = lanes.iter().map(|lane| u16::from(*lane)).sum();
        format!(
            "combined={combined:02x} acc={acc:x} lanes={l0:02x} {l1:02x} {l2:02x} {l3:02x}\n",
            l0 = lanes[0],
            l1 = lanes[1],
            l2 = lanes[2],
            l3 = lanes[3],
        )
    }

    /// Strict IEEE 1364-2001 scope: `reg`/`wire`, old-style function,
    /// `always @*`, generate-for and a clocked accumulator selectable with
    /// `--edition 2001`.
    #[test]
    fn rtl_composition_edition_scope_2001() {
        let expected = edition_scope_expected();
        sim_cli::run_case_with_args(
            SUITE,
            "edition_scope_2001",
            &expected,
            "",
            &[],
            &["--edition", "2001"],
        );
    }

    /// IEEE 1800-2009 scope of the same datapath: `logic`, typed automatic
    /// function, inline generate loop, `always_comb`/`always_ff`, selected with
    /// `--edition 2009`. The oracle is shared with the 2001 variant.
    #[test]
    fn rtl_composition_edition_scope_2009() {
        let expected = edition_scope_expected();
        sim_cli::run_case_with_args(
            SUITE,
            "edition_scope_2009",
            &expected,
            "",
            &[],
            &["--edition", "2009"],
        );
    }

    // ---- rtl_no_silent_omissions defect witnesses ----
    //
    // These are positive compositions that expose real lowerer defects found by
    // this slice. They are ignored only as checked-in evidence: do not convert
    // them into expected rejections. Each carries the LRM-correct oracle so it
    // will pass unchanged once the owning layer is fixed.

    #[test]
    #[ignore = "G1-34 defect: packed struct assigned to a packed member of an \
                unpacked struct (expressions/aggregates.rs, collection/lvalues.rs)"]
    fn defect_member_packed_assign() {
        sim_cli::run_case(
            SUITE,
            "defect_member_packed_assign",
            "u=aa55 1122 3\n",
            "",
            &[],
        );
    }

    #[test]
    #[ignore = "G1-34 defect: fixed unpacked array as a function input formal \
                (collection/arguments.rs, collection/signatures.rs)"]
    fn defect_array_formal() {
        sim_cli::run_case(SUITE, "defect_array_formal", "sum=10\n", "", &[]);
    }

    #[test]
    #[ignore = "G1-34 defect: signed function return does not context-size the \
                RHS (expressions/dispatch.rs, statements/assignments.rs)"]
    fn defect_signed_return() {
        sim_cli::run_case(
            SUITE,
            "defect_signed_return",
            "f1=160 f2=256 g1=160\n",
            "",
            &[],
        );
    }
}
