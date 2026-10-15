//! Transition bins (IEEE 1800-2009 19.5.1, 19.5.4, 19.5.5) as runtime
//! tables, built at compile time.
//!
//! The keys of every transition item split the coverpoint's values into
//! classes: maximal key ranges that each item contains entirely or not at
//! all. A step accepts a set of classes, so a sample is classified once and
//! every pending attempt advances by a bit test. Each transition set is a
//! nondeterministic automaton over (step, repetition count) states; the
//! runtime keeps one attempt per state (the latest start) and starts an
//! attempt at every sample whose class the set's first step accepts.
//!
//! The coverable count of a bin (19.11.1) is decided here: a scalar bin
//! counts when one of its sequences survives the removal of ignored and
//! illegal sequences, and an array bin counts its distinct surviving
//! sequences. Both work on classes, which every item treats alike.
use std::collections::{HashSet, VecDeque};

use crate::sim::ir::{
    IrCoverBinKind, IrCoverRepeat, IrCoverTransBin, IrCoverTransSet, IrCoverTransStep,
    IrCoverTransitions,
};

/// The most automaton states of one coverpoint's transition bins; the
/// runtime keeps per-instance arrays of this size.
pub(super) const MAX_TRANSITION_STATES: u64 = 1 << 16;
/// The most fixed-length sequences one bins array or removal bin expands
/// to through `[* m:n]` ranges.
pub(super) const MAX_EXPANDED_SEQUENCES: u64 = 1 << 12;
/// The most (class, set) start entries of one coverpoint.
pub(super) const MAX_START_ENTRIES: u64 = 1 << 20;
/// The most class sequences one coverpoint's coverable counts may examine.
pub(super) const MAX_CLASS_SEQUENCES: u64 = 1 << 20;

/// One `trans_range_list`: the key ranges of its items and its repetition.
#[derive(Clone, Debug)]
pub(super) struct StepSpec {
    pub(super) ranges: Vec<(u64, u64)>,
    pub(super) repeat: IrCoverRepeat,
    pub(super) lo: u32,
    pub(super) hi: u32,
}

/// One transition bins declaration (a `default sequence` has no sets).
#[derive(Clone, Debug)]
pub(super) struct BinSpec {
    pub(super) name: String,
    pub(super) site: String,
    pub(super) kind: IrCoverBinKind,
    pub(super) array: bool,
    pub(super) guard: Option<u32>,
    pub(super) sets: Vec<Vec<StepSpec>>,
}

/// One thread's outcome after a sample: a state to keep, or a complete
/// sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Event {
    State(usize, u32),
    Done,
}

struct Tables {
    bounds: Vec<u64>,
    words: usize,
    accept: Vec<u64>,
    steps: Vec<IrCoverTransStep>,
}

impl Tables {
    fn accepts(&self, step: usize, class: usize) -> bool {
        (self.accept[step * self.words + class / 64] >> (class % 64)) & 1 == 1
    }

    fn classes(&self, step: usize) -> Vec<usize> {
        (0..self.bounds.len())
            .filter(|class| self.accepts(step, *class))
            .collect()
    }

    /// The number of keys of a class.
    fn size(&self, class: usize, max_key: u64) -> u128 {
        let end = self
            .bounds
            .get(class + 1)
            .map_or(u128::from(max_key) + 1, |bound| u128::from(*bound));
        end - u128::from(self.bounds[class])
    }

    /// Advance one thread at `step` with count `count` by a sample of
    /// `class`; mirrors `llg_cov_consume` in `coverage/coverage.c`.
    fn consume(&self, step: usize, count: u32, class: usize, out: &mut Vec<Event>) {
        let spec = &self.steps[step];
        let accepted = self.accepts(step, class);
        let done = |out: &mut Vec<Event>| {
            out.push(if spec.last {
                Event::Done
            } else {
                Event::State(step + 1, 0)
            })
        };
        match spec.repeat {
            IrCoverRepeat::Consecutive => {
                if !accepted {
                    return;
                }
                if count + 1 < spec.hi {
                    out.push(Event::State(step, count + 1));
                }
                if count + 1 >= spec.lo {
                    done(out);
                }
            }
            IrCoverRepeat::GoTo => {
                if !accepted {
                    out.push(Event::State(step, count));
                    return;
                }
                if count + 1 < spec.hi {
                    out.push(Event::State(step, count + 1));
                }
                if count + 1 >= spec.lo {
                    done(out);
                }
            }
            IrCoverRepeat::Nonconsecutive => {
                if count < spec.lo {
                    out.push(Event::State(step, count + u32::from(accepted)));
                    return;
                }
                // Enough repetitions: the next item may occur at this
                // sample, which ends the gap (decision S45-D8).
                let begins = self.accepts(step + 1, class);
                if begins {
                    self.consume(step + 1, 0, class, out);
                }
                if accepted {
                    if count < spec.hi {
                        out.push(Event::State(step, count + 1));
                    }
                } else if !begins {
                    out.push(Event::State(step, count));
                }
            }
        }
    }
}

/// The fixed-length class-set sequences of a set without goto or
/// nonconsecutive repetition: every `[* m:n]` choice spelled out, as step
/// indices per position.
fn expand(
    steps: &[usize],
    tables_steps: &[IrCoverTransStep],
    limit: u64,
) -> Option<Vec<Vec<usize>>> {
    let mut shapes: Vec<Vec<usize>> = vec![Vec::new()];
    for &step in steps {
        let spec = &tables_steps[step];
        let mut next = Vec::new();
        for shape in &shapes {
            for count in spec.lo..=spec.hi {
                let mut longer = shape.clone();
                longer.extend(std::iter::repeat_n(step, count as usize));
                next.push(longer);
                if next.len() as u64 > limit {
                    return None;
                }
            }
        }
        shapes = next;
    }
    Some(shapes)
}

/// Whether a removal shape matches the classes of `window` ending at its
/// last element.
fn removed_at_end(tables: &Tables, removals: &[Vec<usize>], window: &[usize]) -> bool {
    removals.iter().any(|shape| {
        shape.len() <= window.len()
            && shape
                .iter()
                .zip(&window[window.len() - shape.len()..])
                .all(|(step, class)| tables.accepts(*step, *class))
    })
}

/// Build the transition tables of one coverpoint of `width` bits. An error
/// names the bin (by index) that exceeds a limit.
pub(super) fn build(width: u32, bins: Vec<BinSpec>) -> Result<IrCoverTransitions, (usize, String)> {
    let max_key = if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    };
    let mut bounds = vec![0u64];
    for bin in &bins {
        for step in bin.sets.iter().flatten() {
            for &(lo, hi) in &step.ranges {
                bounds.push(lo);
                if hi < max_key {
                    bounds.push(hi + 1);
                }
            }
        }
    }
    bounds.sort_unstable();
    bounds.dedup();
    let classes = bounds.len();
    let class_of = |key: u64| bounds.partition_point(|bound| *bound <= key) - 1;
    let words = classes.div_ceil(64);

    // Array bins have one bin per sequence (19.5.1): spell out their `[*
    // m:n]` ranges so each set has one length.
    let mut tables = Tables {
        bounds: bounds.clone(),
        words,
        accept: Vec::new(),
        steps: Vec::new(),
    };
    let mut sets = Vec::new();
    let mut states = 0u64;
    let mut window = 0u32;
    let mut default_bin = None;
    for (index, bin) in bins.iter().enumerate() {
        if bin.sets.is_empty() {
            default_bin = Some(index as u32);
        }
        for set in &bin.sets {
            let shapes: Vec<Vec<(usize, u32, u32, IrCoverRepeat)>> = if bin.array {
                let mut shapes: Vec<Vec<(usize, u32, u32, IrCoverRepeat)>> = vec![Vec::new()];
                for (position, step) in set.iter().enumerate() {
                    let mut next = Vec::new();
                    for shape in &shapes {
                        for count in step.lo..=step.hi {
                            let mut longer = shape.clone();
                            longer.extend(std::iter::repeat_n(
                                (position, 1, 1, IrCoverRepeat::Consecutive),
                                count as usize,
                            ));
                            next.push(longer);
                        }
                    }
                    if next.len() as u64 > MAX_EXPANDED_SEQUENCES {
                        return Err((
                            index,
                            format!(
                                "expands to more than {MAX_EXPANDED_SEQUENCES} transition sequences"
                            ),
                        ));
                    }
                    shapes = next;
                }
                shapes
            } else {
                vec![set
                    .iter()
                    .enumerate()
                    .map(|(position, step)| (position, step.lo, step.hi, step.repeat))
                    .collect()]
            };
            for shape in shapes {
                let set_index = sets.len() as u32;
                let first_step = tables.steps.len() as u32;
                let length = shape.len();
                if bin.array {
                    window = window.max(length as u32);
                }
                for (offset, (position, lo, hi, repeat)) in shape.into_iter().enumerate() {
                    let last = offset + 1 == length;
                    // A nonconsecutive repetition that ends its set ends at
                    // its last occurrence, as a goto repetition does
                    // (`3 [= 2]` is `...=>3...=>3`).
                    let repeat = if last && repeat == IrCoverRepeat::Nonconsecutive {
                        IrCoverRepeat::GoTo
                    } else {
                        repeat
                    };
                    let local = match repeat {
                        IrCoverRepeat::Nonconsecutive => u64::from(hi) + 1,
                        _ => u64::from(hi),
                    };
                    tables.steps.push(IrCoverTransStep {
                        repeat,
                        last,
                        lo,
                        hi,
                        state: states as u32,
                        set: set_index,
                    });
                    states += local;
                    if states > MAX_TRANSITION_STATES {
                        return Err((
                            index,
                            format!(
                                "needs more than {MAX_TRANSITION_STATES} transition states in its coverpoint"
                            ),
                        ));
                    }
                    let mut bits = vec![0u64; words];
                    for &(lo, hi) in &set[position].ranges {
                        for class in class_of(lo)..=class_of(hi) {
                            bits[class / 64] |= 1 << (class % 64);
                        }
                    }
                    tables.accept.extend(bits);
                }
                sets.push(IrCoverTransSet {
                    bin: index as u32,
                    first_step,
                    steps: length as u32,
                });
            }
        }
    }

    let mut start_lists = vec![Vec::new(); classes];
    let mut entries = 0u64;
    for (index, set) in sets.iter().enumerate() {
        for class in tables.classes(set.first_step as usize) {
            start_lists[class].push(index as u32);
            entries += 1;
        }
        if entries > MAX_START_ENTRIES {
            return Err((
                set.bin as usize,
                format!("needs more than {MAX_START_ENTRIES} transition start entries in its coverpoint"),
            ));
        }
    }
    let mut start_index = Vec::with_capacity(classes + 1);
    let mut start_sets = Vec::with_capacity(entries as usize);
    for list in start_lists {
        start_index.push(start_sets.len() as u32);
        start_sets.extend(list);
    }
    start_index.push(start_sets.len() as u32);

    // Ignored and illegal sequences have bounded lengths (checked by the
    // caller), so each expands to fixed class-set shapes.
    let mut removals = Vec::new();
    for set in &sets {
        let kind = bins[set.bin as usize].kind;
        if !matches!(kind, IrCoverBinKind::Ignore | IrCoverBinKind::Illegal) {
            continue;
        }
        let steps: Vec<usize> = (set.first_step..set.first_step + set.steps)
            .map(|step| step as usize)
            .collect();
        let shapes = expand(&steps, &tables.steps, MAX_EXPANDED_SEQUENCES).ok_or_else(|| {
            (
                set.bin as usize,
                format!("expands to more than {MAX_EXPANDED_SEQUENCES} transition sequences"),
            )
        })?;
        removals.extend(shapes);
    }
    let removal_length = removals.iter().map(Vec::len).max().unwrap_or(0);

    let mut budget = MAX_CLASS_SEQUENCES;
    let mut ir_bins = Vec::with_capacity(bins.len());
    for (index, bin) in bins.into_iter().enumerate() {
        let bin_sets: Vec<&IrCoverTransSet> = sets
            .iter()
            .filter(|set| set.bin as usize == index)
            .collect();
        let coverable = if bin.kind != IrCoverBinKind::Bins {
            Some(0)
        } else if bin.array {
            array_sequences(&tables, &bin_sets, &removals, max_key, &mut budget)
        } else if removals.is_empty() {
            Some(u64::from(bin_sets.iter().any(|set| {
                (set.first_step..set.first_step + set.steps)
                    .all(|step| !tables.classes(step as usize).is_empty())
            })))
        } else {
            scalar_survives(&tables, &bin_sets, &removals, removal_length, &mut budget)
        }
        .ok_or_else(|| {
            (
                index,
                format!(
                    "needs more than {MAX_CLASS_SEQUENCES} value class sequences to count its sequences"
                ),
            )
        })?;
        ir_bins.push(IrCoverTransBin {
            name: bin.name,
            site: bin.site,
            kind: bin.kind,
            array: bin.array,
            guard: bin.guard,
            coverable,
        });
    }
    Ok(IrCoverTransitions {
        bounds,
        words: words as u32,
        accept: tables.accept,
        steps: tables.steps,
        sets,
        start_index,
        start_sets,
        states: states as u32,
        window,
        bins: ir_bins,
        default_bin,
    })
}

/// The number of distinct sequences of an array bin that contain no
/// removed sequence; `None` past the budget.
fn array_sequences(
    tables: &Tables,
    sets: &[&IrCoverTransSet],
    removals: &[Vec<usize>],
    max_key: u64,
    budget: &mut u64,
) -> Option<u64> {
    let mut seen: HashSet<Vec<usize>> = HashSet::new();
    let mut total: u128 = 0;
    for set in sets {
        let positions: Vec<Vec<usize>> = (set.first_step..set.first_step + set.steps)
            .map(|step| tables.classes(step as usize))
            .collect();
        if positions.iter().any(Vec::is_empty) {
            continue;
        }
        let mut choice = vec![0usize; positions.len()];
        'tuples: loop {
            *budget = budget.checked_sub(1)?;
            let tuple: Vec<usize> = choice
                .iter()
                .zip(&positions)
                .map(|(pick, classes)| classes[*pick])
                .collect();
            let removed =
                (1..=tuple.len()).any(|end| removed_at_end(tables, removals, &tuple[..end]));
            if !removed && seen.insert(tuple.clone()) {
                let size = tuple
                    .iter()
                    .map(|class| tables.size(*class, max_key))
                    .fold(1u128, |product, size| product.saturating_mul(size));
                total = total.saturating_add(size);
            }
            for position in (0..choice.len()).rev() {
                choice[position] += 1;
                if choice[position] < positions[position].len() {
                    continue 'tuples;
                }
                choice[position] = 0;
            }
            break;
        }
    }
    Some(u64::try_from(total).unwrap_or(u64::MAX))
}

/// Whether some sequence of a scalar bin contains no removed sequence
/// (19.5.4: "any covered sequence is removed when it cannot be matched
/// without also matching an ignored sequence"). Searches (state, the last
/// classes of the attempt) pairs; `None` past the budget.
fn scalar_survives(
    tables: &Tables,
    sets: &[&IrCoverTransSet],
    removals: &[Vec<usize>],
    removal_length: usize,
    budget: &mut u64,
) -> Option<u64> {
    type Node = (usize, u32, Vec<usize>);
    let keep = removal_length.saturating_sub(1);
    let classes = tables.bounds.len();
    let mut events = Vec::new();
    for set in sets {
        let mut visited: HashSet<Node> = HashSet::new();
        let mut queue: VecDeque<Node> = VecDeque::new();
        // One sample of `class` for the thread at (step, count) with the
        // attempt's latest classes `window`.
        let mut advance = |step: usize,
                           count: u32,
                           window: &[usize],
                           class: usize,
                           queue: &mut VecDeque<Node>|
         -> Option<bool> {
            *budget = budget.checked_sub(1)?;
            let mut extended = window.to_vec();
            extended.push(class);
            if removed_at_end(tables, removals, &extended) {
                return Some(false);
            }
            if extended.len() > keep {
                extended.drain(..extended.len() - keep);
            }
            events.clear();
            tables.consume(step, count, class, &mut events);
            for event in events.drain(..) {
                match event {
                    Event::Done => return Some(true),
                    Event::State(next, next_count) => {
                        let node = (next, next_count, extended.clone());
                        if visited.insert(node.clone()) {
                            queue.push_back(node);
                        }
                    }
                }
            }
            Some(false)
        };
        let first = set.first_step as usize;
        for class in tables.classes(first) {
            if advance(first, 0, &[], class, &mut queue)? {
                return Some(1);
            }
        }
        while let Some((step, count, window)) = queue.pop_front() {
            for class in 0..classes {
                if advance(step, count, &window, class, &mut queue)? {
                    return Some(1);
                }
            }
        }
    }
    Some(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(ranges: &[(u64, u64)], repeat: IrCoverRepeat, lo: u32, hi: u32) -> StepSpec {
        StepSpec {
            ranges: ranges.to_vec(),
            repeat,
            lo,
            hi,
        }
    }

    fn plain(value: u64) -> StepSpec {
        step(&[(value, value)], IrCoverRepeat::Consecutive, 1, 1)
    }

    fn bin(name: &str, kind: IrCoverBinKind, array: bool, sets: Vec<Vec<StepSpec>>) -> BinSpec {
        BinSpec {
            name: name.to_owned(),
            site: String::new(),
            kind,
            array,
            guard: None,
            sets,
        }
    }

    #[test]
    fn array_bins_count_distinct_sequences() {
        // 19.5.1: sb[] = (4=>5=>6), ([7:9],10=>11,12) has 1 + 8 bins.
        let tables = build(
            4,
            vec![bin(
                "sb",
                IrCoverBinKind::Bins,
                true,
                vec![
                    vec![plain(4), plain(5), plain(6)],
                    vec![
                        step(&[(7, 9), (10, 10)], IrCoverRepeat::Consecutive, 1, 1),
                        step(&[(11, 11), (12, 12)], IrCoverRepeat::Consecutive, 1, 1),
                    ],
                    vec![plain(4), plain(5), plain(6)],
                ],
            )],
        )
        .unwrap();
        assert_eq!(tables.bins[0].coverable, 9);
        assert_eq!(tables.window, 3);
    }

    #[test]
    fn consecutive_ranges_expand_in_arrays() {
        // 3 [* 3:5] is (3=>3=>3), (3=>3=>3=>3), (3=>3=>3=>3=>3).
        let tables = build(
            4,
            vec![bin(
                "r",
                IrCoverBinKind::Bins,
                true,
                vec![vec![step(&[(3, 3)], IrCoverRepeat::Consecutive, 3, 5)]],
            )],
        )
        .unwrap();
        assert_eq!(tables.bins[0].coverable, 3);
        assert_eq!(tables.sets.len(), 3);
        assert_eq!(tables.window, 5);
    }

    #[test]
    fn removal_follows_19_5_4() {
        // The ignored 2=>3 removes 1=>2=>3=>4 but not 1=>2 or [1:2]=>3.
        let ignore = bin(
            "skip",
            IrCoverBinKind::Ignore,
            false,
            vec![vec![plain(2), plain(3)]],
        );
        let tables = build(
            4,
            vec![
                bin(
                    "long",
                    IrCoverBinKind::Bins,
                    false,
                    vec![vec![plain(1), plain(2), plain(3), plain(4)]],
                ),
                bin(
                    "short",
                    IrCoverBinKind::Bins,
                    false,
                    vec![vec![plain(1), plain(2)]],
                ),
                bin(
                    "some",
                    IrCoverBinKind::Bins,
                    false,
                    vec![vec![
                        step(&[(1, 2)], IrCoverRepeat::Consecutive, 1, 1),
                        plain(3),
                    ]],
                ),
                bin(
                    "each",
                    IrCoverBinKind::Bins,
                    true,
                    vec![vec![
                        step(&[(1, 2)], IrCoverRepeat::Consecutive, 1, 1),
                        plain(3),
                    ]],
                ),
                ignore,
            ],
        )
        .unwrap();
        let coverable: Vec<u64> = tables.bins.iter().map(|bin| bin.coverable).collect();
        assert_eq!(coverable, [0, 1, 1, 1, 0]);
    }

    #[test]
    fn goto_sequences_survive_through_gaps() {
        // 1 => 3 [-> 2] avoids the ignored 3=>3 when another value separates
        // the threes; 1 => 3 [* 2] cannot.
        let tables = build(
            4,
            vec![
                bin(
                    "goto",
                    IrCoverBinKind::Bins,
                    false,
                    vec![vec![plain(1), step(&[(3, 3)], IrCoverRepeat::GoTo, 2, 2)]],
                ),
                bin(
                    "run",
                    IrCoverBinKind::Bins,
                    false,
                    vec![vec![
                        plain(1),
                        step(&[(3, 3)], IrCoverRepeat::Consecutive, 2, 2),
                    ]],
                ),
                bin(
                    "skip",
                    IrCoverBinKind::Ignore,
                    false,
                    vec![vec![plain(3), plain(3)]],
                ),
            ],
        )
        .unwrap();
        let coverable: Vec<u64> = tables.bins.iter().map(|bin| bin.coverable).collect();
        assert_eq!(coverable, [1, 0, 0]);
    }

    #[test]
    fn nonconsecutive_gap_ends_at_the_next_item() {
        // 1 => 2 [= 1] => 5: after 1 2, a 5 must take the next item.
        let tables = build(
            4,
            vec![bin(
                "b",
                IrCoverBinKind::Bins,
                false,
                vec![vec![
                    plain(1),
                    step(&[(2, 2)], IrCoverRepeat::Nonconsecutive, 1, 1),
                    plain(5),
                ]],
            )],
        )
        .unwrap();
        let five = tables.bounds.partition_point(|bound| *bound <= 5) - 1;
        let gap = tables.bounds.partition_point(|bound| *bound <= 9) - 1;
        let view = Tables {
            bounds: tables.bounds.clone(),
            words: tables.words as usize,
            accept: tables.accept.clone(),
            steps: tables.steps.clone(),
        };
        let mut events = Vec::new();
        view.consume(1, 1, five, &mut events);
        assert_eq!(events, [Event::Done]);
        events.clear();
        view.consume(1, 1, gap, &mut events);
        assert_eq!(events, [Event::State(1, 1)]);
    }
}
