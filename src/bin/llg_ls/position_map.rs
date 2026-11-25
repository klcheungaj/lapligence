//! Where one text of a document has the positions of another.
//!
//! An analysis names positions of the text it compiled, and the editor shows a
//! text that may have been edited since. [`PositionMap`] relates the two by
//! their lines, which end as the analysis counts them ([`SourcePositions`]).
//! Line endings, indentation and white space at the end of a line do not
//! distinguish lines. Columns are UTF-16 code units.
//!
//! A place is mapped only where the two texts show which line became which:
//!
//! * The lines at the beginning and at the end that both texts have.
//! * Between them, the lines that occur once in both texts and in the same
//!   order, and then what adjoins those lines, as above. A line that occurs
//!   several times says nothing about which of them it is, so it maps only
//!   where its neighbours do.
//! * Of an edited line, its unchanged first and last characters. Edited lines
//!   belong together when as many replace as were there and each resembles its
//!   counterpart most, or when one line was only split or several were only
//!   joined.
//!
//! Every other place is gone: the map has no answer rather than a likely one.

use std::collections::HashMap;
use std::ops::Range as Span;
use std::sync::Arc;

use llg::core::tokens::SourcePositions;
use tower_lsp::lsp_types::{Position, Range};

use crate::features::utf16_byte_offset;

/// Lines of both texts that the search for unchanged lines may look at,
/// repetitions included. Over the rest of this budget, a part of the texts
/// keeps only the equal lines at its beginning and end. Two texts of the
/// default size limit have about 60,000 lines of the usual length.
const MAX_DIFF_LINES: usize = 1 << 18;

/// Edited lines in a row whose counterparts are looked for, which compares each
/// with each.
const MAX_PAIRED_LINES: usize = 16;

/// Lines of the current text per entry of the index that finds a line.
const LINE_INDEX_STRIDE: usize = 64;

/// Lines that both texts have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Run {
    analysed: u32,
    current: u32,
    lines: u32,
}

/// Characters that both texts have in lines that differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Piece {
    analysed: Position,
    current: Position,
    length: u32,
}

/// Maps positions between the text an analysis compiled and the current text
/// of the same document.
#[derive(Debug)]
pub(crate) struct PositionMap {
    /// Disjoint and in the order of both texts, like `pieces`.
    runs: Vec<Run>,
    pieces: Vec<Piece>,
    complete: bool,
    current: Arc<String>,
    /// The offsets of every [`LINE_INDEX_STRIDE`]th line of `current`.
    line_index: Vec<usize>,
}

impl PositionMap {
    pub(crate) fn new(analysed: &str, current: Arc<String>) -> Self {
        let analysed_lines: Vec<&str> = SourcePositions::new(analysed).lines().collect();
        let current_lines: Vec<&str> = SourcePositions::new(&current).lines().collect();
        let line_index = current_lines
            .iter()
            .step_by(LINE_INDEX_STRIDE)
            .map(|line| line.as_ptr() as usize - current.as_ptr() as usize)
            .collect();
        let mut diff = Diff {
            analysed: &analysed_lines,
            current: &current_lines,
            runs: Vec::new(),
            pieces: Vec::new(),
            lines_left: MAX_DIFF_LINES,
            complete: true,
        };
        diff.run();
        let Diff {
            runs,
            pieces,
            complete,
            ..
        } = diff;
        Self {
            runs,
            pieces,
            complete,
            current,
            line_index,
        }
    }

    /// Whether the search looked at every line.
    pub(crate) fn is_complete(&self) -> bool {
        self.complete
    }

    /// The analysed positions that `position` of the current text may stand
    /// for: the place of the character there, then the place after the one
    /// before it, where a name may end.
    pub(crate) fn analysed(&self, position: Position) -> impl Iterator<Item = Position> {
        let at = self.analysed_character(position);
        let after = position
            .character
            .checked_sub(1)
            .and_then(|character| self.analysed_character(Position::new(position.line, character)))
            .map(|place| Position::new(place.line, place.character + 1))
            .filter(|place| Some(*place) != at);
        at.into_iter().chain(after)
    }

    fn analysed_character(&self, position: Position) -> Option<Position> {
        let after = self
            .runs
            .partition_point(|run| run.current <= position.line);
        let run = self.runs[..after].last();
        if let Some(run) = run.filter(|run| position.line - run.current < run.lines) {
            let line = run.analysed + (position.line - run.current);
            return Some(Position::new(line, position.character));
        }
        let after = self
            .pieces
            .partition_point(|piece| piece.current <= position);
        let piece = self.pieces[..after].last()?;
        let offset = piece.offset(piece.current, position)?;
        Some(Position::new(
            piece.analysed.line,
            piece.analysed.character + offset,
        ))
    }

    fn analysed_run(&self, line: u32) -> Option<&Run> {
        let after = self.runs.partition_point(|run| run.analysed <= line);
        self.runs[..after]
            .last()
            .filter(|run| line - run.analysed < run.lines)
    }

    /// The last piece that begins at the analysed `position` or before it.
    fn analysed_piece(&self, position: Position) -> Option<&Piece> {
        let after = self
            .pieces
            .partition_point(|piece| piece.analysed <= position);
        self.pieces[..after].last()
    }

    /// Where the current text has the characters of an analysed `range`, of
    /// which whole words stay whole words. `None` when they are gone.
    pub(crate) fn current(&self, range: Range) -> Option<Range> {
        if let Some(run) = self.analysed_run(range.start.line) {
            let inside = range.end.line.checked_sub(run.analysed)? < run.lines;
            let line = |line: u32| run.current + (line - run.analysed);
            return inside.then(|| {
                Range::new(
                    Position::new(line(range.start.line), range.start.character),
                    Position::new(line(range.end.line), range.end.character),
                )
            });
        }
        let piece = self.analysed_piece(range.start)?;
        let start = piece.offset(piece.analysed, range.start)?;
        let length = range
            .end
            .character
            .checked_sub(range.start.character)
            .filter(|length| {
                range.end.line == range.start.line && start + length <= piece.length
            })?;
        let place = |offset| Position::new(piece.current.line, piece.current.character + offset);
        let end = start + length;
        let range = Range::new(place(start), place(end));
        let extended = (start == 0 && self.is_inside_word(range.start))
            || (end == piece.length && self.is_inside_word(range.end));
        (!extended).then_some(range)
    }

    /// Where a stretch of analysed text that ended at `end` ends now, when its
    /// last characters may be gone.
    pub(crate) fn current_end(&self, end: Position) -> Option<Position> {
        if let Some(run) = self.analysed_run(end.line) {
            let line = run.current + (end.line - run.analysed);
            return Some(Position::new(line, end.character));
        }
        let run = self.runs.partition_point(|run| run.analysed <= end.line);
        let run_end = self.runs[..run].last().and_then(|run| {
            let line = run.current + run.lines - 1;
            Some(Position::new(line, utf16_len(self.line(line)?)))
        });
        let piece_end = self.analysed_piece(end).map(|piece| {
            let length = if piece.analysed.line == end.line {
                piece.length.min(end.character - piece.analysed.character)
            } else {
                piece.length
            };
            Position::new(piece.current.line, piece.current.character + length)
        });
        run_end.max(piece_end)
    }

    /// A line of the current text without its line ending.
    pub(crate) fn line(&self, line: u32) -> Option<&str> {
        let line = line as usize;
        let block = line / LINE_INDEX_STRIDE;
        let start = *self.line_index.get(block)?;
        let end = self.line_index.get(block + 1).copied();
        let text = &self.current[start..end.unwrap_or(self.current.len())];
        let lines = SourcePositions::new(text);
        let found = lines.lines().nth(line % LINE_INDEX_STRIDE);
        found
    }

    /// Whether the characters around `position` of the current text belong to
    /// one word.
    fn is_inside_word(&self, position: Position) -> bool {
        let Some(line) = self.line(position.line) else {
            return false;
        };
        let (before, after) = line.split_at(utf16_byte_offset(line, position.character));
        before.ends_with(is_word) && after.starts_with(is_word)
    }
}

impl Piece {
    /// The offset of `position` in this piece, which begins at `start` in the
    /// text of the position.
    fn offset(&self, start: Position, position: Position) -> Option<u32> {
        let offset = position.character.checked_sub(start.character)?;
        (position.line == start.line && offset < self.length).then_some(offset)
    }
}

/// A character of an identifier or keyword.
pub(crate) fn is_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_' || character == '$'
}

fn utf16_len(text: &str) -> u32 {
    if text.is_ascii() {
        text.len() as u32
    } else {
        text.encode_utf16().count() as u32
    }
}

/// The search for what both texts have, in the order of the texts.
struct Diff<'t> {
    analysed: &'t [&'t str],
    current: &'t [&'t str],
    runs: Vec<Run>,
    pieces: Vec<Piece>,
    /// What is left of [`MAX_DIFF_LINES`].
    lines_left: usize,
    complete: bool,
}

/// What the search has still to look at or to note.
enum Part {
    /// This many equal lines from these on.
    Equal(usize, usize, usize),
    Open(Span<usize>, Span<usize>),
}

fn same_line(left: &str, right: &str) -> bool {
    left == right || left.trim_ascii() == right.trim_ascii()
}

impl<'t> Diff<'t> {
    fn run(&mut self) {
        let mut pending = vec![Part::Open(0..self.analysed.len(), 0..self.current.len())];
        while let Some(part) = pending.pop() {
            let (mut analysed, mut current) = match part {
                Part::Equal(line, place, lines) => {
                    self.add_equal(line, place, lines);
                    continue;
                }
                Part::Open(analysed, current) => (analysed, current),
            };
            let equal = |(left, right): &(&&str, &&str)| same_line(left, right);
            let prefix = self.analysed[analysed.clone()]
                .iter()
                .zip(&self.current[current.clone()])
                .take_while(equal)
                .count();
            self.add_equal(analysed.start, current.start, prefix);
            analysed.start += prefix;
            current.start += prefix;
            let suffix = self.analysed[analysed.clone()]
                .iter()
                .rev()
                .zip(self.current[current.clone()].iter().rev())
                .take_while(equal)
                .count();
            analysed.end -= suffix;
            current.end -= suffix;
            let (anchors, contested) = self.anchors(&analysed, &current, suffix);
            if anchors.is_empty() {
                let (line, place) = (analysed.end, current.end);
                self.add_edited(analysed, current);
                self.add_equal(line, place, suffix);
                continue;
            }
            analysed.end += contested;
            current.end += contested;
            let certain = Part::Equal(analysed.end, current.end, suffix - contested);
            pending.push(certain);
            let first = pending.len();
            for (line, place) in anchors {
                if analysed.start < line || current.start < place {
                    pending.push(Part::Open(analysed.start..line, current.start..place));
                }
                pending.push(Part::Equal(line, place, 1));
                analysed.start = line + 1;
                current.start = place + 1;
            }
            pending.push(Part::Open(analysed, current));
            pending[first..].reverse();
        }
        debug_assert!(self.runs.is_sorted_by_key(|run| run.analysed));
        debug_assert!(self.pieces.is_sorted_by_key(|piece| piece.analysed));
    }

    /// Note `lines` lines that are equal from these on.
    fn add_equal(&mut self, analysed: usize, current: usize, lines: usize) {
        for offset in 0..lines {
            let (line, place) = (analysed + offset, current + offset);
            let (from, to) = (self.analysed[line], self.current[place]);
            if from == to {
                match self.runs.last_mut() {
                    Some(run)
                        if (run.analysed + run.lines) as usize == line
                            && (run.current + run.lines) as usize == place =>
                    {
                        run.lines += 1;
                    }
                    _ => self.runs.push(Run {
                        analysed: line as u32,
                        current: place as u32,
                        lines: 1,
                    }),
                }
            } else {
                let indentation = |line: &str| line.len() - line.trim_ascii_start().len();
                let length = from.trim_ascii().len();
                self.add_piece((line, indentation(from)), (place, indentation(to)), length);
            }
        }
    }

    /// Note `length` equal bytes from these lines and byte columns on.
    fn add_piece(&mut self, analysed: (usize, usize), current: (usize, usize), length: usize) {
        if length == 0 {
            return;
        }
        let (from, to) = (self.analysed[analysed.0], self.current[current.0]);
        let column = |line: &str, (number, bytes): (usize, usize)| {
            Position::new(number as u32, utf16_len(&line[..bytes]))
        };
        self.pieces.push(Piece {
            analysed: column(from, analysed),
            current: column(to, current),
            length: utf16_len(&from[analysed.1..analysed.1 + length]),
        });
    }

    /// The lines that occur once in both spans, in the order of both. Those
    /// that others would contradict are left out ([`uncontested`]).
    ///
    /// The first lines of the `suffix` equal ones after the spans may belong to
    /// equal lines in the spans instead. As many of them as do so count as part
    /// of the spans and are returned as well.
    fn anchors(
        &mut self,
        analysed: &Span<usize>,
        current: &Span<usize>,
        suffix: usize,
    ) -> (Vec<(usize, usize)>, usize) {
        if analysed.is_empty() || current.is_empty() {
            return (Vec::new(), 0);
        }
        match self.lines_left.checked_sub(analysed.len() + current.len()) {
            Some(left) => self.lines_left = left,
            None => {
                self.complete = false;
                return (Vec::new(), 0);
            }
        }
        let mut lines = Lines::with_capacity(analysed.len().max(current.len()));
        for line in analysed.clone() {
            lines
                .entry(self.analysed[line].trim_ascii())
                .in_analysed(line);
        }
        for line in current.clone() {
            lines
                .entry(self.current[line].trim_ascii())
                .in_current(line);
        }
        let mut contested = 0;
        while contested < suffix {
            let (line, place) = (analysed.end + contested, current.end + contested);
            let Some(occurrences) = lines.get_mut(self.analysed[line].trim_ascii()) else {
                break;
            };
            occurrences.in_analysed(line);
            occurrences.in_current(place);
            contested += 1;
        }
        // These are in the order of the analysed text: each that has a place
        // there and is not repeated was found there first.
        let pairs: Vec<(usize, usize)> = lines
            .found
            .iter()
            .filter(|found| !found.repeated)
            .filter_map(|found| Some((found.analysed?, found.current?)))
            // Punctuation and blank lines mark no place.
            .filter(|(line, _)| self.analysed[*line].contains(is_word))
            .collect();
        debug_assert!(pairs.is_sorted());
        (uncontested(pairs), contested)
    }

    /// Note what is unchanged of the lines in `analysed`, which those in
    /// `current` replaced.
    fn add_edited(&mut self, analysed: Span<usize>, current: Span<usize>) {
        if analysed.is_empty() || current.is_empty() {
            return;
        }
        let (first, first_place) = (analysed.start, current.start);
        let (last, last_place) = (analysed.end - 1, current.end - 1);
        if analysed.len() == 1 || current.len() == 1 {
            let prefix = common_prefix(self.analysed[first], self.current[first_place]);
            let (from, to) = (self.analysed[last], self.current[last_place]);
            // A single line has the first characters too.
            let from_rest = &from[if analysed.len() == 1 { prefix } else { 0 }..];
            let to_rest = &to[if current.len() == 1 { prefix } else { 0 }..];
            let suffix = common_suffix(from_rest, to_rest);
            let is_blank = |rest: &str| rest[..rest.len() - suffix].trim_ascii().is_empty();
            let only_split = analysed.len() == 1 && is_blank(from_rest);
            let only_joined = current.len() == 1 && is_blank(to_rest);
            if analysed.len() == current.len() || only_split || only_joined {
                self.add_piece((first, 0), (first_place, 0), prefix);
                let (start, place) = (from.len() - suffix, to.len() - suffix);
                self.add_piece((last, start), (last_place, place), suffix);
            }
        } else if analysed.len() == current.len() && analysed.len() <= MAX_PAIRED_LINES {
            for (line, place) in analysed.clone().zip(current.clone()) {
                let own = self.resemblance(line, place);
                let closest = analysed
                    .clone()
                    .filter(|other| *other != line)
                    .all(|other| self.resemblance(other, place) < own)
                    && current
                        .clone()
                        .filter(|other| *other != place)
                        .all(|other| self.resemblance(line, other) < own);
                if closest {
                    self.add_edited(line..line + 1, place..place + 1);
                }
            }
        }
    }

    /// How many bytes of both ends the two lines have in common, the
    /// indentation apart.
    fn resemblance(&self, analysed: usize, current: usize) -> usize {
        let (from, to) = (
            self.analysed[analysed].trim_ascii(),
            self.current[current].trim_ascii(),
        );
        let prefix = common_prefix(from, to);
        prefix + common_suffix(&from[prefix..], &to[prefix..])
    }
}

/// Where a line occurs in the spans that are compared.
#[derive(Default)]
struct Occurrences {
    analysed: Option<usize>,
    current: Option<usize>,
    /// Whether it occurs several times in one of them.
    repeated: bool,
}

impl Occurrences {
    fn in_analysed(&mut self, line: usize) {
        self.repeated |= self.analysed.replace(line).is_some();
    }

    fn in_current(&mut self, line: usize) {
        self.repeated |= self.current.replace(line).is_some();
    }
}

/// Distinct lines, in the order in which they were found.
struct Lines<'t> {
    numbers: HashMap<&'t str, usize>,
    found: Vec<Occurrences>,
}

impl<'t> Lines<'t> {
    fn with_capacity(lines: usize) -> Self {
        Self {
            numbers: HashMap::with_capacity(lines),
            found: Vec::with_capacity(lines),
        }
    }

    fn entry(&mut self, line: &'t str) -> &mut Occurrences {
        let number = *self.numbers.entry(line).or_insert(self.found.len());
        if number == self.found.len() {
            self.found.push(Occurrences::default());
        }
        &mut self.found[number]
    }

    fn get_mut(&mut self, line: &str) -> Option<&mut Occurrences> {
        Some(&mut self.found[*self.numbers.get(line)?])
    }
}

fn common_prefix(left: &str, right: &str) -> usize {
    let mut length = left
        .bytes()
        .zip(right.bytes())
        .take_while(|(left, right)| left == right)
        .count();
    while !(left.is_char_boundary(length) && right.is_char_boundary(length)) {
        length -= 1;
    }
    length
}

fn common_suffix(left: &str, right: &str) -> usize {
    let mut length = left
        .bytes()
        .rev()
        .zip(right.bytes().rev())
        .take_while(|(left, right)| left == right)
        .count();
    while !(left.is_char_boundary(left.len() - length)
        && right.is_char_boundary(right.len() - length))
    {
        length -= 1;
    }
    length
}

/// The pairs of the longest chain that ascends in both members, without those
/// that other pairs contradict: text that moved past other text leaves two
/// chains, of which only a clearly longer one says what stayed in place.
/// `pairs` ascend in their first member, and no member repeats.
fn uncontested(pairs: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    if pairs.is_sorted_by_key(|pair| pair.1) {
        return pairs;
    }
    let mut tails: Vec<usize> = Vec::new();
    let mut previous = vec![None; pairs.len()];
    for (index, pair) in pairs.iter().enumerate() {
        let length = tails.partition_point(|tail| pairs[*tail].1 < pair.1);
        previous[index] = length.checked_sub(1).map(|before| tails[before]);
        if length == tails.len() {
            tails.push(index);
        } else {
            tails[length] = index;
        }
    }
    let mut chain = Vec::with_capacity(tails.len());
    let mut next = tails.last().copied();
    while let Some(index) = next {
        chain.push(pairs[index]);
        next = previous[index];
    }
    chain.reverse();
    // For each link, how many other pairs contradict it, and how many links
    // those contradict, as differences between neighbours.
    let mut changes = vec![(0_i64, 0_i64); chain.len() + 1];
    let mut links = chain.iter().peekable();
    for pair in &pairs {
        if links.next_if_eq(&pair).is_some() {
            continue;
        }
        let by_analysed = chain.partition_point(|link| link.0 < pair.0);
        let by_current = chain.partition_point(|link| link.1 < pair.1);
        let contradicted = by_analysed.min(by_current)..by_analysed.max(by_current);
        let length = contradicted.len() as i64;
        changes[contradicted.start].0 += 1;
        changes[contradicted.start].1 += length;
        changes[contradicted.end].0 -= 1;
        changes[contradicted.end].1 -= length;
    }
    let (mut against, mut contradicted) = (0, 0);
    chain
        .into_iter()
        .zip(changes)
        .filter_map(|(link, change)| {
            against += change.0;
            contradicted += change.1;
            // Twice as many links as pairs that contradict them.
            (2 * against * against <= contradicted).then_some(link)
        })
        .collect()
}

#[cfg(test)]
mod tests;
