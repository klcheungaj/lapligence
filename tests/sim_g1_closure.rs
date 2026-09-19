//! Group 1 closure scenarios for integral state conversion, enum navigation,
//! fill-literal context, data/array queries, and arithmetic/expression sizing.
//!
//! Every fixture is checked in under `tests/fixtures/sim/feature_completion/`
//! and runs through the `llg` executable with and without optimization. The
//! expected traces are built here from independent LRM-derived oracles; no
//! expected value is captured from the simulator.

#[path = "support/sim.rs"]
mod sim_harness;

#[path = "support/sim_cli.rs"]
mod sim_cli;

mod g1_10 {
    use super::sim_cli;

    const WIDTHS: &[usize] = &[1, 31, 32, 33, 63, 64, 65, 129, 256];

    /// IEEE 1800-2009 6.24.1: truncate or extend (`sign`-filled only when the
    /// source is signed), then clear X/Z when the destination is two-state.
    fn convert(source: &str, source_signed: bool, target: usize, two_state: bool) -> String {
        let mut bits: Vec<char> = if source.len() >= target {
            source.chars().skip(source.len() - target).collect()
        } else {
            let fill = if source_signed {
                source.chars().next().unwrap()
            } else {
                '0'
            };
            let mut value: Vec<char> = std::iter::repeat_n(fill, target - source.len()).collect();
            value.extend(source.chars());
            value
        };
        if two_state {
            for bit in &mut bits {
                if matches!(bit, 'x' | 'z') {
                    *bit = '0';
                }
            }
        }
        bits.into_iter().collect()
    }

    #[test]
    fn integral_default_matrix() {
        use std::fmt::Write as _;
        let mut expected = String::new();
        for &width in WIDTHS {
            writeln!(expected, "d4_{width}={}", "x".repeat(width)).unwrap();
            writeln!(expected, "d2_{width}={}", "0".repeat(width)).unwrap();
            writeln!(
                expected,
                "c4_{width}={}",
                convert("z1x0z1x0", false, width, false)
            )
            .unwrap();
            writeln!(
                expected,
                "c2_{width}={}",
                convert("z1x0z1x0", false, width, true)
            )
            .unwrap();
            writeln!(
                expected,
                "s4_{width}={}",
                convert("1x0z", true, width, false)
            )
            .unwrap();
            writeln!(
                expected,
                "s2_{width}={}",
                convert("1x0z", true, width, true)
            )
            .unwrap();
        }
        expected.push_str("PASS integral_default_matrix\n");
        sim_cli::run_case(
            "feature_completion/g1_10",
            "integral_default_matrix",
            &expected,
            "",
            &[],
        );
    }

    // IEEE 1800-2009 6.19.5: declaration-order member values.
    const MEMBERS: &[(&str, &str)] = &[
        ("E_MIN", "10000000"),
        ("E_NEG", "11111111"),
        ("E_ZERO", "00000000"),
        ("E_POS", "00110010"),
        ("E_MAX", "01111111"),
    ];
    const TWO_MEMBERS: &[(&str, &str)] = &[("T_LO", "001"), ("T_MID", "100"), ("T_HI", "111")];

    fn navigate(members: &[(&str, &str)], index: usize, step: u64, forward: bool) -> String {
        let count = members.len() as u64;
        let offset = step % count;
        let target = if forward {
            (index as u64 + offset) % count
        } else {
            (index as u64 + count - offset) % count
        };
        members[target as usize].1.to_owned()
    }

    #[test]
    fn enum_navigation_sparse() {
        use std::fmt::Write as _;
        let unknown4 = "x".repeat(8);
        let mut expected = String::new();
        writeln!(
            expected,
            "first={} last={} num={}",
            MEMBERS[0].1,
            MEMBERS[4].1,
            MEMBERS.len()
        )
        .unwrap();
        writeln!(
            expected,
            "min name={} next={} prev={}",
            MEMBERS[0].0,
            navigate(MEMBERS, 0, 1, true),
            navigate(MEMBERS, 0, 1, false)
        )
        .unwrap();
        writeln!(
            expected,
            "max name={} next={} prev={}",
            MEMBERS[4].0,
            navigate(MEMBERS, 4, 1, true),
            navigate(MEMBERS, 4, 1, false)
        )
        .unwrap();
        writeln!(
            expected,
            "zero name={} next0={} next1={} next4={} next5={} next6={}",
            MEMBERS[2].0,
            navigate(MEMBERS, 2, 0, true),
            navigate(MEMBERS, 2, 1, true),
            navigate(MEMBERS, 2, 4, true),
            navigate(MEMBERS, 2, 5, true),
            navigate(MEMBERS, 2, 6, true)
        )
        .unwrap();
        writeln!(
            expected,
            "zero prev0={} prev1={} prev4={} prev6={}",
            navigate(MEMBERS, 2, 0, false),
            navigate(MEMBERS, 2, 1, false),
            navigate(MEMBERS, 2, 4, false),
            navigate(MEMBERS, 2, 6, false)
        )
        .unwrap();
        writeln!(
            expected,
            "zero nextx={} prevx={}",
            navigate(MEMBERS, 2, 0, true),
            navigate(MEMBERS, 2, 0, false)
        )
        .unwrap();
        writeln!(expected, "invalid name=[] next={unknown4} prev={unknown4}").unwrap();
        writeln!(expected, "highz name=[] next={unknown4} prev={unknown4}").unwrap();
        writeln!(expected, "partial name=[] next={unknown4} prev={unknown4}").unwrap();
        writeln!(
            expected,
            "two mid name={} next={} prev={} num={}",
            TWO_MEMBERS[1].0,
            navigate(TWO_MEMBERS, 1, 1, true),
            navigate(TWO_MEMBERS, 1, 1, false),
            TWO_MEMBERS.len()
        )
        .unwrap();
        writeln!(expected, "two invalid name=[] next=000 prev=000").unwrap();
        writeln!(
            expected,
            "two invalid first={} last={}",
            TWO_MEMBERS[0].1, TWO_MEMBERS[2].1
        )
        .unwrap();
        expected.push_str("PASS enum_navigation_sparse\n");
        sim_cli::run_case(
            "feature_completion/g1_10",
            "enum_navigation_sparse",
            &expected,
            "",
            &[],
        );
    }

    #[test]
    fn fill_literal_context() {
        let expected = concat!(
            "member hi=0000 lo=1111 whole=00001111\n",
            "isolate hi=0000 lo=1111\n",
            "part high=0000 low=00000000\n",
            "lanes0=00000000 lanes1=11111111\n",
            "eight=xxxxxxxx\n",
            "or=1111\n",
            "ternary=11111111\n",
            "fn=0011001111001100\n",
            "concat=110100\n",
            "bits=1\n",
            "PASS fill_literal_context\n",
        );
        sim_cli::run_case(
            "feature_completion/g1_10",
            "fill_literal_context",
            expected,
            "",
            &[],
        );
    }

    #[test]
    fn enum_method_arity_is_rejected() {
        sim_cli::reject_case(
            "feature_completion/g1_10",
            "enum_method_arity",
            "too many arguments",
        );
    }

    #[test]
    fn fill_literal_string_is_rejected() {
        sim_cli::reject_case(
            "feature_completion/g1_10",
            "fill_literal_string",
            "no implicit conversion",
        );
    }
}

mod g1_27 {
    use super::sim_cli;

    #[derive(Clone, Copy)]
    struct Dim {
        left: i128,
        right: i128,
    }

    const fn dim(left: i128, right: i128) -> Dim {
        Dim { left, right }
    }

    fn increment(dimension: Dim) -> i128 {
        if dimension.left >= dimension.right {
            1
        } else {
            -1
        }
    }

    fn low(dimension: Dim) -> i128 {
        dimension.left.min(dimension.right)
    }

    fn high(dimension: Dim) -> i128 {
        dimension.left.max(dimension.right)
    }

    fn size(dimension: Dim) -> i128 {
        (dimension.left - dimension.right).unsigned_abs() as i128 + 1
    }

    fn query_line(label: &str, dimension: Dim) -> String {
        format!(
            "{label}={} {} {} {} {} {}",
            dimension.left,
            dimension.right,
            low(dimension),
            high(dimension),
            increment(dimension),
            size(dimension)
        )
    }

    #[test]
    fn query_dimension_order() {
        use std::fmt::Write as _;
        let mut expected = String::new();
        let n: Vec<Dim> = vec![dim(1, 5), dim(2, 8), dim(3, 0), dim(2, 1)];
        assert_eq!(n.len(), 4);
        writeln!(expected, "n dims=4 unpacked=2").unwrap();
        for (index, dimension) in n.iter().copied().enumerate() {
            writeln!(
                expected,
                "{}",
                query_line(&format!("n[{}]", index + 1), dimension)
            )
            .unwrap();
        }
        let fixed = [dim(7, 4), dim(7, 0)];
        writeln!(expected, "fixed dims=2 unpacked=1").unwrap();
        for (index, dimension) in fixed.iter().copied().enumerate() {
            writeln!(
                expected,
                "{}",
                query_line(&format!("fixed[{}]", index + 1), dimension)
            )
            .unwrap();
        }
        let asc = [dim(0, 1), dim(0, 7)];
        writeln!(expected, "asc dims=2 unpacked=1").unwrap();
        for (index, dimension) in asc.iter().copied().enumerate() {
            writeln!(
                expected,
                "{}",
                query_line(&format!("asc[{}]", index + 1), dimension)
            )
            .unwrap();
        }
        // IEEE 1800-2009 20.7: a simple bit-vector nonarray type reports one
        // dimension even at one bit.
        writeln!(expected, "scalar dims=1 unpacked=0").unwrap();
        writeln!(expected, "scalar_two dims=1 unpacked=0").unwrap();
        writeln!(expected, "bit_struct dims=1 unpacked=0").unwrap();
        writeln!(expected, "Word size={} dims=1", size(dim(8, 1))).unwrap();
        writeln!(
            expected,
            "ram dims=2 unpacked=1 size2={} left={}",
            size(dim(8, 1)),
            dim(0, 9).left
        )
        .unwrap();
        expected.push_str("PASS query_dimension_order\n");
        sim_cli::run_case(
            "feature_completion/g1_27",
            "query_dimension_order",
            &expected,
            "",
            &[],
        );
    }

    #[test]
    fn query_constant_runtime_parity() {
        let expected = concat!(
            "arr dims=2+2 unpacked=1+1 left=7+7 right=4+4 low=4+4 high=7+7 inc=1+1 size=4+4 bits=32+32\n",
            "n dims=4+4 size2=7+7\n",
            "p bits=8+8\n",
            "clog2=3+3\n",
            "isunbounded=0+0 unbounded=1+1\n",
            "PASS query_constant_runtime_parity\n",
        );
        sim_cli::run_case(
            "feature_completion/g1_27",
            "query_constant_runtime_parity",
            expected,
            "",
            &[],
        );
    }

    #[test]
    fn query_invalid_dimension() {
        let expected = concat!(
            "zero left=x size=x\n",
            "high left=x size=x\n",
            "dyn1 left=0 size=2\n",
            "dyn3 left=x size=x\n",
            "PASS query_invalid_dimension\n",
        );
        sim_cli::run_case(
            "feature_completion/g1_27",
            "query_invalid_dimension",
            expected,
            "",
            &[],
        );
    }

    #[test]
    fn query_constant_out_of_range_dimension_is_rejected() {
        sim_cli::reject_case(
            "feature_completion/g1_27",
            "query_zero_dimension",
            "not a valid dimension",
        );
    }
}

mod g1_11 {
    use super::sim_cli;
    use std::cmp::Ordering;
    use std::fmt::Write as _;

    // ---- four-state 2-bit truth-table oracle ----

    fn decode(index: usize) -> (u8, u8) {
        ((index / 4) as u8, (index % 4) as u8)
    }

    fn known(bit: u8) -> bool {
        bit <= 1
    }

    fn numeric(value: (u8, u8)) -> Option<u32> {
        if known(value.0) && known(value.1) {
            Some(u32::from(value.0) * 2 + u32::from(value.1))
        } else {
            None
        }
    }

    fn glyph(bit: u8) -> char {
        match bit {
            0 => '0',
            1 => '1',
            2 => 'x',
            _ => 'z',
        }
    }

    fn pair(high: u8, low: u8) -> String {
        format!("{}{}", glyph(high), glyph(low))
    }

    fn number(value: u32) -> String {
        pair(((value >> 1) & 1) as u8, (value & 1) as u8)
    }

    fn bit_and(a: u8, b: u8) -> u8 {
        const TABLE: [[u8; 4]; 4] = [[0, 0, 0, 0], [0, 1, 2, 2], [0, 2, 2, 2], [0, 2, 2, 2]];
        TABLE[a as usize][b as usize]
    }

    fn bit_or(a: u8, b: u8) -> u8 {
        const TABLE: [[u8; 4]; 4] = [[0, 1, 2, 2], [1, 1, 1, 1], [2, 1, 2, 2], [2, 1, 2, 2]];
        TABLE[a as usize][b as usize]
    }

    fn bit_xor(a: u8, b: u8) -> u8 {
        const TABLE: [[u8; 4]; 4] = [[0, 1, 2, 2], [1, 0, 2, 2], [2, 2, 2, 2], [2, 2, 2, 2]];
        TABLE[a as usize][b as usize]
    }

    fn op_and(a: (u8, u8), b: (u8, u8)) -> String {
        pair(bit_and(a.0, b.0), bit_and(a.1, b.1))
    }

    fn op_or(a: (u8, u8), b: (u8, u8)) -> String {
        pair(bit_or(a.0, b.0), bit_or(a.1, b.1))
    }

    fn op_xor(a: (u8, u8), b: (u8, u8)) -> String {
        pair(bit_xor(a.0, b.0), bit_xor(a.1, b.1))
    }

    fn op_xnor(a: (u8, u8), b: (u8, u8)) -> String {
        let flip = |bit: u8| if bit <= 1 { 1 - bit } else { 2 };
        pair(flip(bit_xor(a.0, b.0)), flip(bit_xor(a.1, b.1)))
    }

    fn op_add(a: (u8, u8), b: (u8, u8)) -> String {
        match (numeric(a), numeric(b)) {
            (Some(x), Some(y)) => number((x + y) & 3),
            _ => "xx".to_owned(),
        }
    }

    fn op_sub(a: (u8, u8), b: (u8, u8)) -> String {
        match (numeric(a), numeric(b)) {
            (Some(x), Some(y)) => number((x + 4 - y) & 3),
            _ => "xx".to_owned(),
        }
    }

    fn op_mul(a: (u8, u8), b: (u8, u8)) -> String {
        match (numeric(a), numeric(b)) {
            (Some(x), Some(y)) => number((x * y) & 3),
            _ => "xx".to_owned(),
        }
    }

    fn op_div(a: (u8, u8), b: (u8, u8)) -> String {
        match (numeric(a), numeric(b)) {
            (Some(x), Some(y)) if y != 0 => number(x / y),
            _ => "xx".to_owned(),
        }
    }

    fn op_mod(a: (u8, u8), b: (u8, u8)) -> String {
        match (numeric(a), numeric(b)) {
            (Some(x), Some(y)) if y != 0 => number(x % y),
            _ => "xx".to_owned(),
        }
    }

    fn op_eq(a: (u8, u8), b: (u8, u8)) -> String {
        let mut unknown = false;
        for (left, right) in [(a.0, b.0), (a.1, b.1)] {
            if known(left) && known(right) {
                if left != right {
                    return "0".to_owned();
                }
            } else {
                unknown = true;
            }
        }
        if unknown {
            "x".to_owned()
        } else {
            "1".to_owned()
        }
    }

    fn op_ne(a: (u8, u8), b: (u8, u8)) -> String {
        match op_eq(a, b).as_str() {
            "0" => "1".to_owned(),
            "1" => "0".to_owned(),
            _ => "x".to_owned(),
        }
    }

    fn op_ceq(a: (u8, u8), b: (u8, u8)) -> String {
        if a == b { "1" } else { "0" }.to_owned()
    }

    fn op_cne(a: (u8, u8), b: (u8, u8)) -> String {
        if a == b { "0" } else { "1" }.to_owned()
    }

    fn relation(a: (u8, u8), b: (u8, u8), predicate: impl Fn(u32, u32) -> bool) -> String {
        match (numeric(a), numeric(b)) {
            (Some(x), Some(y)) => if predicate(x, y) { "1" } else { "0" }.to_owned(),
            _ => "x".to_owned(),
        }
    }

    fn op_lt(a: (u8, u8), b: (u8, u8)) -> String {
        relation(a, b, |x, y| x < y)
    }

    fn op_le(a: (u8, u8), b: (u8, u8)) -> String {
        relation(a, b, |x, y| x <= y)
    }

    fn op_gt(a: (u8, u8), b: (u8, u8)) -> String {
        relation(a, b, |x, y| x > y)
    }

    fn op_ge(a: (u8, u8), b: (u8, u8)) -> String {
        relation(a, b, |x, y| x >= y)
    }

    #[derive(PartialEq)]
    enum Tri {
        True,
        False,
        Unknown,
    }

    fn tri(value: (u8, u8)) -> Tri {
        if value.0 == 1 || value.1 == 1 {
            Tri::True
        } else if known(value.0) && known(value.1) {
            Tri::False
        } else {
            Tri::Unknown
        }
    }

    fn op_land(a: (u8, u8), b: (u8, u8)) -> String {
        match (tri(a), tri(b)) {
            (Tri::False, _) | (_, Tri::False) => "0",
            (Tri::True, Tri::True) => "1",
            _ => "x",
        }
        .to_owned()
    }

    fn op_lor(a: (u8, u8), b: (u8, u8)) -> String {
        match (tri(a), tri(b)) {
            (Tri::True, _) | (_, Tri::True) => "1",
            (Tri::False, Tri::False) => "0",
            _ => "x",
        }
        .to_owned()
    }

    fn op_shl(a: (u8, u8), b: (u8, u8)) -> String {
        match numeric(b) {
            Some(shift) if shift >= 2 => "00".to_owned(),
            Some(1) => pair(a.1, 0),
            Some(_) => pair(a.0, a.1),
            None => "xx".to_owned(),
        }
    }

    fn op_shr(a: (u8, u8), b: (u8, u8)) -> String {
        match numeric(b) {
            Some(shift) if shift >= 2 => "00".to_owned(),
            Some(1) => pair(0, a.0),
            Some(_) => pair(a.0, a.1),
            None => "xx".to_owned(),
        }
    }

    type Ops = &'static [(&'static str, fn((u8, u8), (u8, u8)) -> String)];

    const SMALL_OPS: Ops = &[
        ("and", op_and),
        ("or", op_or),
        ("xor", op_xor),
        ("xnor", op_xnor),
        ("add", op_add),
        ("sub", op_sub),
        ("mul", op_mul),
        ("div", op_div),
        ("mod", op_mod),
        ("eq", op_eq),
        ("ne", op_ne),
        ("ceq", op_ceq),
        ("cne", op_cne),
        ("lt", op_lt),
        ("le", op_le),
        ("gt", op_gt),
        ("ge", op_ge),
        ("land", op_land),
        ("lor", op_lor),
        ("shl", op_shl),
        ("shr", op_shr),
    ];

    #[test]
    fn arithmetic_small_exhaustive() {
        let mut expected = String::new();
        for ai in 0..16 {
            for bi in 0..16 {
                let a = decode(ai);
                let b = decode(bi);
                for (name, operation) in SMALL_OPS {
                    writeln!(expected, "{ai} {bi} {name} {}", operation(a, b)).unwrap();
                }
            }
        }
        expected.push_str("PASS arithmetic_small_exhaustive\n");
        sim_cli::run_case(
            "feature_completion/g1_11",
            "arithmetic_small_exhaustive",
            &expected,
            "",
            &[],
        );
    }

    // ---- arbitrary-width two's-complement oracle ----

    type Bits = Vec<bool>; // little-endian

    fn set(width: usize, ones: &[usize]) -> Bits {
        let mut value = vec![false; width];
        for &bit in ones {
            value[bit] = true;
        }
        value
    }

    fn render(value: &Bits) -> String {
        value
            .iter()
            .rev()
            .map(|bit| if *bit { '1' } else { '0' })
            .collect()
    }

    fn ensure(value: &Bits, width: usize, signed: bool) -> Bits {
        let mut out = value.clone();
        let fill = signed && value.last().copied().unwrap_or(false);
        out.resize(width, fill);
        out
    }

    fn add(a: &[bool], b: &[bool]) -> Bits {
        let mut out = vec![false; a.len()];
        let mut carry = false;
        for index in 0..a.len() {
            let sum = u8::from(a[index]) + u8::from(b[index]) + u8::from(carry);
            out[index] = sum & 1 == 1;
            carry = sum >= 2;
        }
        out
    }

    fn sub(a: &[bool], b: &[bool]) -> Bits {
        let complement: Bits = b.iter().map(|bit| !bit).collect();
        let mut one = vec![false; a.len()];
        one[0] = true;
        add(&add(a, &complement), &one)
    }

    fn mul(a: &[bool], b: &[bool]) -> Bits {
        let mut accumulator = vec![false; a.len()];
        for (index, bit) in b.iter().enumerate() {
            if *bit {
                accumulator = add(&accumulator, &shift_left(a, index));
            }
        }
        accumulator
    }

    fn shift_left(value: &[bool], count: usize) -> Bits {
        let mut out = vec![false; value.len()];
        if count < value.len() {
            out[count..].copy_from_slice(&value[..value.len() - count]);
        }
        out
    }

    fn shift_right_arithmetic(value: &[bool], count: usize, signed: bool) -> Bits {
        let fill = signed && value.last().copied().unwrap_or(false);
        let mut out = vec![fill; value.len()];
        let shift = count.min(value.len());
        out[..value.len() - shift].copy_from_slice(&value[shift..]);
        out
    }

    fn compare_unsigned(a: &[bool], b: &[bool]) -> Ordering {
        for index in (0..a.len()).rev() {
            match (a[index], b[index]) {
                (true, false) => return Ordering::Greater,
                (false, true) => return Ordering::Less,
                _ => {}
            }
        }
        Ordering::Equal
    }

    fn compare_signed(a: &[bool], b: &[bool]) -> Ordering {
        match (
            a.last().copied().unwrap_or(false),
            b.last().copied().unwrap_or(false),
        ) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => compare_unsigned(a, b),
        }
    }

    fn common(a: &Bits, a_signed: bool, b: &Bits, b_signed: bool) -> (Bits, Bits, bool) {
        let width = a.len().max(b.len());
        let signed = a_signed && b_signed;
        (ensure(a, width, signed), ensure(b, width, signed), signed)
    }

    fn binary(kind: u8, a: &Bits, a_signed: bool, b: &Bits, b_signed: bool, dest: usize) -> String {
        let common_width = a.len().max(b.len());
        let signed = a_signed && b_signed;
        // IEEE 1800-2009 11.8.2: the assignment width propagates down to the
        // context-determined operands when it is wider than the operands.
        let op_width = common_width.max(dest);
        let left = ensure(a, op_width, signed);
        let right = ensure(b, op_width, signed);
        let result = match kind {
            b'a' => add(&left, &right),
            b's' => sub(&left, &right),
            b'm' => mul(&left, &right),
            b'&' => left.iter().zip(&right).map(|(x, y)| *x && *y).collect(),
            b'|' => left.iter().zip(&right).map(|(x, y)| *x || *y).collect(),
            b'^' => left.iter().zip(&right).map(|(x, y)| x != y).collect(),
            _ => unreachable!(),
        };
        render(&ensure(&result, dest, signed))
    }

    fn comparison(
        a: &Bits,
        a_signed: bool,
        b: &Bits,
        b_signed: bool,
        predicate: impl Fn(Ordering) -> bool,
    ) -> String {
        let (left, right, signed) = common(a, a_signed, b, b_signed);
        let ordering = if signed {
            compare_signed(&left, &right)
        } else {
            compare_unsigned(&left, &right)
        };
        if predicate(ordering) { "1" } else { "0" }.to_owned()
    }

    fn shift(
        a: &Bits,
        a_signed: bool,
        count: usize,
        left: bool,
        arithmetic: bool,
        dest: usize,
    ) -> String {
        // The assignment context resizes the (context-determined) left operand
        // before the shift; iverilog agrees with this reading.
        let op_width = a.len().max(dest);
        let operand = ensure(a, op_width, a_signed);
        let result = if left {
            shift_left(&operand, count)
        } else {
            shift_right_arithmetic(&operand, count, arithmetic && a_signed)
        };
        render(&ensure(&result, dest, a_signed))
    }

    #[test]
    fn signed_context_wide() {
        let s65 = set(65, &[64, 63, 0]);
        let u65 = set(65, &[63, 0]);
        let s65b = set(65, &[1, 0]);
        let s257 = set(257, &[256, 255, 64, 0]);
        let u257 = set(257, &[255, 0]);
        let s257b = set(257, &[2, 0]);

        let mut expected = String::new();
        writeln!(
            expected,
            "add65 {} {}",
            binary(b'a', &s65, true, &u65, false, 65),
            binary(b'a', &s65, true, &u65, false, 130)
        )
        .unwrap();
        writeln!(
            expected,
            "sub65 {} {}",
            binary(b's', &s65, true, &u65, false, 65),
            binary(b's', &s65, true, &u65, false, 130)
        )
        .unwrap();
        writeln!(
            expected,
            "mulu65 {} {}",
            binary(b'm', &s65, true, &u65, false, 65),
            binary(b'm', &s65, true, &u65, false, 130)
        )
        .unwrap();
        writeln!(
            expected,
            "muls65 {} {}",
            binary(b'm', &s65, true, &s65b, true, 65),
            binary(b'm', &s65, true, &s65b, true, 130)
        )
        .unwrap();
        writeln!(
            expected,
            "and65 {} {}",
            binary(b'&', &s65, true, &u65, false, 65),
            binary(b'&', &s65, true, &u65, false, 130)
        )
        .unwrap();
        writeln!(
            expected,
            "or65 {} {}",
            binary(b'|', &s65, true, &u65, false, 65),
            binary(b'|', &s65, true, &u65, false, 130)
        )
        .unwrap();
        writeln!(
            expected,
            "xor65 {} {}",
            binary(b'^', &s65, true, &u65, false, 65),
            binary(b'^', &s65, true, &u65, false, 130)
        )
        .unwrap();
        writeln!(
            expected,
            "shl65 {} {}",
            shift(&s65, true, 5, true, false, 65),
            shift(&s65, true, 5, true, false, 130)
        )
        .unwrap();
        writeln!(
            expected,
            "ashr65 {} {}",
            shift(&s65, true, 5, false, true, 65),
            shift(&s65, true, 5, false, true, 130)
        )
        .unwrap();
        writeln!(
            expected,
            "cmp65 {} {} {}",
            comparison(&s65, true, &u65, false, |o| o == Ordering::Less),
            comparison(&s65, true, &u65, false, |o| o == Ordering::Greater),
            comparison(&s65, true, &s65b, true, |o| o == Ordering::Less)
        )
        .unwrap();

        writeln!(
            expected,
            "add257 {} {}",
            binary(b'a', &s257, true, &u257, false, 257),
            binary(b'a', &s257, true, &u257, false, 514)
        )
        .unwrap();
        writeln!(
            expected,
            "sub257 {} {}",
            binary(b's', &s257, true, &u257, false, 257),
            binary(b's', &s257, true, &u257, false, 514)
        )
        .unwrap();
        writeln!(
            expected,
            "mulu257 {} {}",
            binary(b'm', &s257, true, &u257, false, 257),
            binary(b'm', &s257, true, &u257, false, 514)
        )
        .unwrap();
        writeln!(
            expected,
            "muls257 {} {}",
            binary(b'm', &s257, true, &s257b, true, 257),
            binary(b'm', &s257, true, &s257b, true, 514)
        )
        .unwrap();
        writeln!(
            expected,
            "and257 {} {}",
            binary(b'&', &s257, true, &u257, false, 257),
            binary(b'&', &s257, true, &u257, false, 514)
        )
        .unwrap();
        writeln!(
            expected,
            "or257 {} {}",
            binary(b'|', &s257, true, &u257, false, 257),
            binary(b'|', &s257, true, &u257, false, 514)
        )
        .unwrap();
        writeln!(
            expected,
            "xor257 {} {}",
            binary(b'^', &s257, true, &u257, false, 257),
            binary(b'^', &s257, true, &u257, false, 514)
        )
        .unwrap();
        writeln!(
            expected,
            "shl257 {} {}",
            shift(&s257, true, 5, true, false, 257),
            shift(&s257, true, 5, true, false, 514)
        )
        .unwrap();
        writeln!(
            expected,
            "ashr257 {} {}",
            shift(&s257, true, 5, false, true, 257),
            shift(&s257, true, 5, false, true, 514)
        )
        .unwrap();
        writeln!(
            expected,
            "cmp257 {} {} {}",
            comparison(&s257, true, &u257, false, |o| o == Ordering::Less),
            comparison(&s257, true, &u257, false, |o| o == Ordering::Greater),
            comparison(&s257, true, &s257b, true, |o| o == Ordering::Less)
        )
        .unwrap();

        expected.push_str("PASS signed_context_wide\n");
        sim_cli::run_case(
            "feature_completion/g1_11",
            "signed_context_wide",
            &expected,
            "",
            &[],
        );
    }

    #[test]
    fn divide_zero_and_shift() {
        let x8 = "x".repeat(8);
        let z8 = "0".repeat(8);
        let o8 = "1".repeat(8);
        let x65 = "x".repeat(65);
        let z65 = "0".repeat(65);
        let o65 = "1".repeat(65);
        let x257 = "x".repeat(257);
        let z257 = "0".repeat(257);
        let expected = format!(
            "small const divz {x8} {x8}\n\
             small runtime divz {x8} {x8}\n\
             small unknown divz {x8} {x8}\n\
             small overshift {z8} {z8} {o8}\n\
             small unknown shift {x8} {x8}\n\
             wide divz={x65} shl={z65} shr={z65} ashr={o65}\n\
             vwide divz={x257} shl={z257} shr={z257}\n\
             vwide unknown shift={x257}\n\
             PASS divide_zero_and_shift\n"
        );
        sim_cli::run_case(
            "feature_completion/g1_11",
            "divide_zero_and_shift",
            &expected,
            "",
            &[],
        );
    }
}
