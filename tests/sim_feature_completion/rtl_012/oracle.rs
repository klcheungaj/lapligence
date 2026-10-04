//! Independent scalar strength oracle (IEEE 1364-2001 7.9-7.13, 17.1.1.5;
//! IEEE 1800-2009 28.11-28.15, 21.2.1.5).
//!
//! Every source is the set of signed strength levels it may take (-7 = Su0,
//! 0 = HiZ, 7 = Su1). The resolved signal is the hull of the outcomes of
//! every combination of choices, found by exhaustive enumeration: the
//! strongest level wins, and equal opposite levels give an X range on a wire
//! (7.10.2) or the wired value on wand/wor (7.10.4). This brute force is
//! deliberately unlike the simulator's strongest-level formula.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bit {
    Zero,
    One,
    X,
    Z,
}

pub fn bit(index: usize) -> Bit {
    [Bit::Zero, Bit::One, Bit::X, Bit::Z][index % 4]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Net {
    Wire,
    Wand,
    Wor,
    Tri0,
    Tri1,
}

#[derive(Clone, Copy, Debug)]
pub enum Source {
    /// A continuous assignment or gate output with `(strength0, strength1)`.
    Drive { s0: i8, s1: i8, value: Bit },
    /// `bufif1` with drive strengths, data and enable (IEEE 1364-2001 Table 35).
    Bufif1 {
        s0: i8,
        s1: i8,
        data: Bit,
        enable: Bit,
    },
}

impl Source {
    fn levels(self) -> Vec<i8> {
        let range = |lo: i8, hi: i8| (lo..=hi).collect::<Vec<_>>();
        match self {
            Source::Drive { s0, s1, value } => match value {
                Bit::Zero => vec![-s0],
                Bit::One => vec![s1],
                Bit::X => range(-s0, s1),
                Bit::Z => vec![0],
            },
            Source::Bufif1 {
                s0,
                s1,
                data,
                enable,
            } => {
                let data = if data == Bit::Z { Bit::X } else { data };
                match enable {
                    Bit::One => Source::Drive {
                        s0,
                        s1,
                        value: data,
                    }
                    .levels(),
                    Bit::Zero => vec![0],
                    Bit::X | Bit::Z => match data {
                        Bit::Zero => range(-s0, 0),
                        Bit::One => range(0, s1),
                        _ => range(-s0, s1),
                    },
                }
            }
        }
    }
}

fn outcome(net: Net, choice: &[i8]) -> (i8, i8) {
    let strongest = choice.iter().map(|level| level.abs()).max().unwrap_or(0);
    if strongest == 0 {
        return (0, 0);
    }
    let one = choice.contains(&strongest);
    let zero = choice.contains(&-strongest);
    match (zero, one, net) {
        (true, true, Net::Wand) => (-strongest, -strongest),
        (true, true, Net::Wor) => (strongest, strongest),
        (true, true, _) => (-strongest, strongest),
        (true, false, _) => (-strongest, -strongest),
        _ => (strongest, strongest),
    }
}

/// The resolved strength range `(lo, hi)` of one net bit.
pub fn resolve(net: Net, sources: &[Source]) -> (i8, i8) {
    let mut sets = sources
        .iter()
        .map(|source| source.levels())
        .collect::<Vec<_>>();
    match net {
        Net::Tri0 => sets.push(vec![-5]),
        Net::Tri1 => sets.push(vec![5]),
        _ => {}
    }
    let mut hull: Option<(i8, i8)> = None;
    let mut index = vec![0usize; sets.len()];
    loop {
        let choice = index
            .iter()
            .zip(&sets)
            .map(|(position, set)| set[*position])
            .collect::<Vec<_>>();
        let (lo, hi) = outcome(net, &choice);
        hull = Some(match hull {
            None => (lo, hi),
            Some((low, high)) => (low.min(lo), high.max(hi)),
        });
        let mut digit = 0;
        loop {
            if digit == index.len() {
                return hull.unwrap_or((0, 0));
            }
            index[digit] += 1;
            if index[digit] < sets[digit].len() {
                break;
            }
            index[digit] = 0;
            digit += 1;
        }
    }
}

const NAMES: [&str; 8] = ["Hi", "Sm", "Me", "We", "La", "Pu", "St", "Su"];

/// `%v` text (IEEE 1364-2001 Tables 69-71).
pub fn strength_text((lo, hi): (i8, i8)) -> String {
    let pair = |first: i8, second: i8, value: char| {
        if first == second {
            format!("{}{value}", NAMES[first as usize])
        } else {
            format!("{first}{second}{value}")
        }
    };
    if lo == 0 && hi == 0 {
        "HiZ".to_owned()
    } else if hi < 0 {
        pair(-lo, -hi, '0')
    } else if lo > 0 {
        pair(hi, lo, '1')
    } else if hi == 0 {
        format!("{}L", NAMES[(-lo) as usize])
    } else if lo == 0 {
        format!("{}H", NAMES[hi as usize])
    } else {
        pair(-lo, hi, 'X')
    }
}

/// `%b` digit of the same signal: L and H read as x.
pub fn value_text((lo, hi): (i8, i8)) -> char {
    if lo == 0 && hi == 0 {
        'z'
    } else if hi < 0 {
        '0'
    } else if lo > 0 {
        '1'
    } else {
        'x'
    }
}

#[test]
fn component_oracle_matches_clause_examples() {
    // 7.10.2 Figure 7: We1 and We0 give WeX.
    let we = |value| Source::Drive {
        s0: 3,
        s1: 3,
        value,
    };
    assert_eq!(
        strength_text(resolve(Net::Wire, &[we(Bit::One), we(Bit::Zero)])),
        "WeX"
    );
    // Figure 12: PuH and WeL combine to 35X.
    let puh = Source::Bufif1 {
        s0: 5,
        s1: 5,
        data: Bit::One,
        enable: Bit::X,
    };
    let wel = Source::Bufif1 {
        s0: 3,
        s1: 3,
        data: Bit::Zero,
        enable: Bit::X,
    };
    assert_eq!(strength_text(resolve(Net::Wire, &[puh, wel])), "35X");
    // Figures 19-22: StH from and(strong1, highz0) with We0 gives 36X.
    let sth = Source::Drive {
        s0: 0,
        s1: 6,
        value: Bit::X,
    };
    assert_eq!(
        strength_text(resolve(Net::Wire, &[sth, we(Bit::Zero)])),
        "36X"
    );
    // 7.13.1: a tri0 net with no driver is Pu0; tri1 with We0 is Pu1.
    assert_eq!(strength_text(resolve(Net::Tri0, &[])), "Pu0");
    assert_eq!(strength_text(resolve(Net::Tri1, &[we(Bit::Zero)])), "Pu1");
    // 7.10.4: equal-strength opposite values follow the wired function.
    assert_eq!(
        strength_text(resolve(Net::Wand, &[we(Bit::One), we(Bit::Zero)])),
        "We0"
    );
    assert_eq!(
        strength_text(resolve(Net::Wor, &[we(Bit::One), we(Bit::Zero)])),
        "We1"
    );
}
