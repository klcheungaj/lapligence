//! One-based line and UTF-16 column positions of byte offsets.
//!
//! Diagnostics report a line and a UTF-16 column (the language server's unit).
//! Counting from the start of the file for every diagnostic cost diagnostics x
//! file size, which made netlists with one warning per instance quadratic. A
//! cursor resumes from the previous offset, so offsets visited in ascending
//! order cost one pass over the file.

/// A resumable scan over one source text. A line ends at `\n`, `\r` or
/// `\r\n`; a column counts UTF-16 code units from 1.
pub(super) struct Utf16Cursor<'t> {
    text: &'t str,
    /// Byte offset scanned so far (a character boundary).
    offset: usize,
    line: u32,
    column: u32,
    /// The scan ended on `\r`, so a following `\n` ends no further line.
    after_cr: bool,
}

impl<'t> Utf16Cursor<'t> {
    pub(super) fn new(text: &'t str) -> Self {
        Self {
            text,
            offset: 0,
            line: 1,
            column: 1,
            after_cr: false,
        }
    }

    /// The position of byte `offset`, clamped to the text and moved back to
    /// a character boundary. An offset before the previous one rescans from
    /// the start of the text.
    pub(super) fn position(&mut self, offset: u64) -> (u32, u32) {
        let mut end = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(self.text.len());
        while !self.text.is_char_boundary(end) {
            end -= 1;
        }
        if end < self.offset {
            *self = Self::new(self.text);
        }
        for character in self.text[self.offset..end].chars() {
            match character {
                '\r' => {
                    self.line = self.line.saturating_add(1);
                    self.column = 1;
                    self.after_cr = true;
                    continue;
                }
                '\n' if self.after_cr => {}
                '\n' => {
                    self.line = self.line.saturating_add(1);
                    self.column = 1;
                }
                _ => self.column = self.column.saturating_add(character.len_utf16() as u32),
            }
            self.after_cr = false;
        }
        self.offset = end;
        (self.line, self.column)
    }
}

/// The positions of `offsets` in `text`, in the order given; the scan visits
/// them in ascending order, so the cost is one pass plus the sort.
pub(super) fn one_based_utf16_positions(text: &str, offsets: &[u64]) -> Vec<(u32, u32)> {
    let mut order: Vec<usize> = (0..offsets.len()).collect();
    order.sort_by_key(|index| offsets[*index]);
    let mut cursor = Utf16Cursor::new(text);
    let mut positions = vec![(0, 0); offsets.len()];
    for index in order {
        positions[index] = cursor.position(offsets[index]);
    }
    positions
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The position definition the cursor must match: a scan of the whole
    /// prefix, as positions were computed before the cursor.
    fn rescan(text: &str, offset: u64) -> (u32, u32) {
        let mut end = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let (mut line, mut column) = (1, 1);
        let mut chars = text[..end].chars().peekable();
        while let Some(character) = chars.next() {
            match character {
                '\r' | '\n' => {
                    line += 1;
                    column = 1;
                    if character == '\r' && chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                }
                _ => column += character.len_utf16() as u32,
            }
        }
        (line, column)
    }

    #[test]
    fn line_ends_and_columns_count_utf16_units() {
        let text = "a😀\r\nb\rc\nd";
        assert_eq!(rescan(text, 0), (1, 1));
        assert_eq!(rescan(text, "a😀".len() as u64), (1, 4));
        // Inside the emoji: moved back to its start.
        assert_eq!(rescan(text, 2), (1, 2));
        // Between `\r` and `\n`, and after the pair: one line break.
        assert_eq!(rescan(text, "a😀\r".len() as u64), (2, 1));
        assert_eq!(rescan(text, "a😀\r\n".len() as u64), (2, 1));
        assert_eq!(rescan(text, "a😀\r\nb\r".len() as u64), (3, 1));
        assert_eq!(rescan(text, "a😀\r\nb\rc\n".len() as u64), (4, 1));
        assert_eq!(rescan(text, u64::MAX), (4, 2));
    }

    #[test]
    fn batches_match_independent_scans_in_any_order() {
        let text = "x\r\n\r\ny😀z\n\rw\r\n";
        let offsets: Vec<u64> = (0..=text.len() as u64 + 1).rev().chain([3, 1, 9]).collect();
        let expected: Vec<_> = offsets.iter().map(|offset| rescan(text, *offset)).collect();
        assert_eq!(one_based_utf16_positions(text, &offsets), expected);
        // A resumed cursor stopped between `\r` and `\n` still joins them.
        let mut cursor = Utf16Cursor::new(text);
        for offset in 0..=text.len() as u64 {
            assert_eq!(
                cursor.position(offset),
                rescan(text, offset),
                "offset {offset}"
            );
        }
    }
}
