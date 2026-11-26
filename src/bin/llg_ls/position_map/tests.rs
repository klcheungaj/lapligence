use super::*;

/// The map from `analysed`, where `«` and `»` enclose what is looked for, to
/// `current`, where they enclose what it must become. Without them in
/// `current` it must be gone.
fn check(analysed: &str, current: &str) {
    let (analysed, range) = marked(analysed);
    let (current, expected) = marked(current);
    let range = range.expect("a marked place");
    let map = PositionMap::new(&analysed, Arc::new(current.clone()));
    assert_eq!(
        map.current(range),
        expected,
        "{range:?} of {analysed:?} in {current:?}: {map:?}"
    );
    if let Some(expected) = expected {
        for (position, place) in [(expected.start, range.start), (expected.end, range.end)] {
            assert!(
                map.analysed(position).any(|found| found == place),
                "{position:?} of {current:?} in {analysed:?}: {map:?}"
            );
        }
    }
}

fn marked(text: &str) -> (String, Option<Range>) {
    let (Some(start), Some(end)) = (text.find('«'), text.find('»')) else {
        return (text.to_owned(), None);
    };
    let name = &text[start + '«'.len_utf8()..end];
    let plain = format!("{}{name}{}", &text[..start], &text[end + '»'.len_utf8()..]);
    let range = Range::new(
        position(&plain, start),
        position(&plain, start + name.len()),
    );
    (plain, Some(range))
}

/// The position of a byte of `text`.
fn position(text: &str, offset: usize) -> Position {
    let before: Vec<&str> = SourcePositions::new(&text[..offset]).lines().collect();
    // The start of a line after CR and before LF is none.
    assert!(!(text[..offset].ends_with('\r') && text[offset..].starts_with('\n')));
    Position::new(
        (before.len() - 1) as u32,
        utf16_len(before[before.len() - 1]),
    )
}

#[test]
fn lines_around_an_edit_keep_their_names() {
    check("a;\n«b»;\nc;\n", "a;\nnew;\nnew2;\n«b»;\nc;\n");
    check("«a»;\nb;\nc;\n", "new;\n«a»;\nb;\nc;\n");
    check("a;\nb;\nc;\nd;\n«e»;\n", "a;\nc;\n«e»;\n");
    check("a;\n«b»;\nc;\n", "a;\nc;\n");
    check("a;\nb;\n«c»;\n", "a;\nnew;\n«c»;");
    check("«a»;\nb;\n", "");
    check("«a»;", "«a»;\n");
}

#[test]
fn edited_lines_keep_the_names_around_the_edit() {
    check("assign «a» = b;\n", "assign «a» = new;\n");
    check("assign a = «b»;\n", "assign new = «b»;\n");
    check("assign a = «b» + c;\n", "assign a = new + c;\n");
    check("assign «a» = b;\n", "assign «a»  = b;\n");
    check("  «x» <= 1;\n", "    «x» <= 1;\n");
    check("\t«x» <= 1;\nz;\n", "x;\n  «x» <= 1;  \nz;\n");
    // Two edits in one line: only its ends are known.
    check("f(a, «b», c);\n", "g(a, b, d);\n");
}

#[test]
fn edited_names_are_gone() {
    check("wire «foo» = 1;\n", "wire foobar = 1;\n");
    check("wire «foo» = 1;\n", "wire barfoo = 1;\n");
    check("wire «foo» = 1;\n", "wire fo = 1;\n");
    check("wire «foo» = 1;\n", "wire f_oo = 1;\n");
    check("wire «foo» = 1;\n", "wire foo$ = 1;\n");
    check("wire «foo» = 1;\n", "wire fooé = 1;\n");
    check("wire «foo» = 1;\n", "wire «foo»+bar = 1;\n");
    check("wire «foo» = 1;\n", "wire bar+«foo» = 1;\n");
}

#[test]
fn splits_and_joins_keep_names() {
    check(
        "assign a = b; assign «c» = d;\n",
        "assign a = b;\nassign «c» = d;\n",
    );
    check(
        "assign «a» = b; assign c = d;\n",
        "assign «a» = b;\n\n  assign c = d;\n",
    );
    // Split and edited: which line it became is a guess.
    check(
        "assign «a» = b; assign c = d;\n",
        "assign a = b;\nassign e = d;\n",
    );
    check(
        "assign a = b; assign «c» = d;\n",
        "assign a = b;\nnew;\n  assign «c» = d;\n",
    );
    check(
        "assign a = b;\nassign «c» = d;\n",
        "assign a = b; assign «c» = d;\n",
    );
    check(
        "assign «a» = b;\nassign c = d;\n",
        "assign «a» = b;assign c = d;\n",
    );
    // The middle of three lines is new.
    check("f(«a», b);\n", "f(«a»,\n  // note\n  b);\n");
    check("wire «foobar» = 1;\n", "wire foo\nbar = 1;\n");
}

#[test]
fn repeated_lines_need_neighbours() {
    // The first or the second one went.
    check("x;\n«y»;\nz;\ny;\nw;\n", "x;\n«y»;\nz;\nw;\n");
    check("a;\nend\n«b»;\nend\nc;\n", "new;\na;\nend\n«b»;\nend\nc;\n");
    check("a;\n«end»\nb;\nend\nc;\n", "new;\na;\n«end»\nb;\nend\nc;\n");
    // Deleted after the first rather than in its middle.
    let modules = "module a;\nlogic «x»;\nend\nmodule b;\nlogic x;\nend\n";
    check(modules, "new;\nmodule a;\nlogic «x»;\nend\n");
    check(modules, "new;\nmodule b;\nlogic x;\nend\n");
    // One more of them, and nothing beside them is unchanged.
    check("p\nx;\nq;\n«x»;\nr\n", "s\nx;\nx;\nx;\nt\n");
}

#[test]
fn moved_text_is_gone_unless_what_stayed_is_clear() {
    check(
        "a1;\na2;\n«a3»;\nb1;\nb2;\nb3;\n",
        "b1;\nb2;\nb3;\na1;\na2;\na3;\n",
    );
    check(
        "a1;\na2;\na3;\n«b1»;\nb2;\nb3;\n",
        "b1;\nb2;\nb3;\na1;\na2;\na3;\n",
    );
    // One line moved past six.
    check(
        "a;\nb;\nc;\nd;\ne;\nf;\n«g»;\nlast;\n",
        "first;\ng;\na;\nb;\nc;\nd;\ne;\nf;\nlast;\n",
    );
    check(
        "a;\nb;\n«c»;\nd;\ne;\nf;\ng;\nlast;\n",
        "first;\ng;\na;\nb;\n«c»;\nd;\ne;\nf;\nlast;\n",
    );
}

#[test]
fn columns_are_utf_16_and_lines_end_as_the_analysis_counts_them() {
    check(
        "string s = \"é𝄞\"; int «n»;\n",
        "new; string s = \"é𝄞\"; int «n»;\n",
    );
    check(
        "string s = \"é\"; int «n» = 1;\n",
        "string s = \"𝄞𝄞\"; int «n» = 1;\n",
    );
    check(
        "string s = \"é\"; int «n» = 1;\n",
        "string s = \"è\"; int «n» = 1;\n",
    );
    check("wire \\a+b ; wire «c»;\n", "wire \\a+bé ; wire «c»;\n");
    check("a;\r\n«b»;\r\nc;\r\n", "a;\nnew;\n«b»;\nc;\n");
    check("a;\n«b»;\nc;\n", "a;\r\nnew;\r\n«b»;\r\nc;\r\n");
    check("a;\r«b»;\rc;\r", "a;\rnew;\r«b»;\rc;\r");
    check("\r\r\n\n«b»;", "\r\n\r\r«b»;");
}

#[test]
fn a_position_stands_for_the_characters_around_it() {
    let map = PositionMap::new("f(aa, b);\n", Arc::new("f(aa, new, b);\n".to_owned()));
    let places = |character| -> Vec<u32> {
        map.analysed(Position::new(0, character))
            .map(|place| place.character)
            .collect()
    };
    assert_eq!(places(2), [2]);
    assert_eq!(places(4), [4]);
    // Between what stayed and what is new.
    assert_eq!(places(6), [6]);
    assert!(places(8).is_empty());
    assert_eq!(places(11), [6]);
    // After the last character of the line.
    assert_eq!(places(14), [9]);
    assert!(places(15).is_empty());
    let beyond: Vec<Position> = map.analysed(Position::new(7, 0)).collect();
    assert_eq!(beyond, []);
}

#[test]
fn a_stretch_ends_with_what_is_left_of_it() {
    let end = |analysed: &str, current: &str, line, character| {
        PositionMap::new(analysed, Arc::new(current.to_owned()))
            .current_end(Position::new(line, character))
    };
    let module = "module m;\n  a;\nendmodule\nmodule n;\nendmodule\n";
    assert_eq!(
        end(module, "x\nmodule m;\n  a;\nendmodule\n", 2, 9),
        Some(Position::new(3, 9))
    );
    // Its last line went.
    assert_eq!(
        end("module m;\n  a;\nendmodule\n", "module m;\n  a;\n", 2, 9),
        Some(Position::new(1, 4))
    );
    assert_eq!(
        end(
            "module m;\n  a;\nendmodule\n",
            "module m;\n  a;\nendmod\n",
            2,
            9
        ),
        Some(Position::new(2, 6))
    );
    assert_eq!(end("a;\nb;\n", "new;\n", 0, 1), None);
}

#[test]
fn what_the_budget_leaves_out_is_gone() {
    let body = |prefix: &str, count: usize| -> String {
        (0..count)
            .map(|line| format!("{prefix}{line};\n"))
            .collect()
    };
    // Both ends are found without a search.
    let half = MAX_DIFF_LINES / 2;
    let analysed = format!("first;\n{}last;\n", body("a", half));
    let current = format!("first;\nnew;\n{}new;\nlast;\n", body("a", half));
    let map = PositionMap::new(&analysed, Arc::new(current));
    assert!(!map.is_complete());
    let name = |line: usize, length| {
        let line = line as u32;
        Range::new(Position::new(line, 0), Position::new(line, length))
    };
    assert_eq!(map.current(name(0, 5)), Some(name(0, 5)));
    assert_eq!(map.current(name(1, 2)), None);
    assert_eq!(map.current(name(half + 1, 4)), Some(name(half + 3, 4)));

    let analysed = format!("first;\n{}last;\n", body("a", half - 8));
    let current = format!("first;\nnew;\n{}new;\nlast;\n", body("a", half - 8));
    let map = PositionMap::new(&analysed, Arc::new(current));
    assert!(map.is_complete());
    assert_eq!(map.current(name(1, 2)), Some(name(2, 2)));
}

/// A text of the size limit with edits at both ends, so that the search looks
/// at every line.
#[test]
fn a_text_of_one_mebibyte_is_searched_completely() {
    let mut lines: Vec<String> = Vec::new();
    let mut bytes = 0;
    while bytes < 1 << 20 {
        let line = match lines.len() % 4 {
            3 => "  end".to_owned(),
            _ => format!("  assign net_{} = net_{} ^ 8'd7;", lines.len(), bytes),
        };
        bytes += line.len() + 1;
        lines.push(line);
    }
    let analysed = lines.join("\n");
    let last = lines.len() - 1;
    lines[0].push_str(" // note");
    lines[last].push_str(" // note");
    lines.insert(last / 2, "  logic fresh;".to_owned());
    let map = PositionMap::new(&analysed, Arc::new(lines.join("\n")));
    assert!(map.is_complete());
    for line in 1..last {
        let range = |line: usize| {
            let line = line as u32;
            Range::new(Position::new(line, 2), Position::new(line, 5))
        };
        let now = if line < last / 2 { line } else { line + 1 };
        assert_eq!(map.current(range(line)), Some(range(now)));
    }
}

/// Numbers that look random (xorshift).
struct Random(u64);

impl Random {
    fn below(&mut self, end: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % end.max(1) as u64) as usize
    }
}

/// A line of a text that is edited, and where it was before that.
#[derive(Clone)]
struct Line {
    origin: Option<usize>,
    text: String,
    ending: &'static str,
}

const ENDINGS: [&str; 3] = ["\n", "\r\n", "\r"];

fn join(lines: &[Line]) -> String {
    lines
        .iter()
        .flat_map(|line| [line.text.as_str(), line.ending])
        .collect()
}

/// Whatever the edits are, every place that has an answer holds the same
/// characters, the answers keep the order of the text, and they lead back.
/// Lines that occur once and were not touched have an answer.
#[test]
fn random_edits_keep_untouched_names_and_invent_none() {
    for seed in 1..=600 {
        let mut random = Random(0x9e37_79b9_7f4a_7c15_u64.wrapping_mul(seed));
        let endings = random.below(ENDINGS.len());
        let repeat = seed % 3 == 0;
        let original: Vec<Line> = (0..random.below(60))
            .map(|line| Line {
                origin: Some(line),
                text: match line % 4 {
                    _ if repeat && random.below(2) == 0 => "  end".to_owned(),
                    0 => format!("  assign a{line} = b{line} + \\esc+{line} ;"),
                    1 => format!("\"é𝄞{line}\" f{line}(x{line}, y{line});"),
                    2 => format!("w{line}"),
                    _ => String::new(),
                },
                ending: ENDINGS[endings],
            })
            .collect();
        let mut lines = original.clone();
        let mut copied = false;
        for edit in 0..random.below(8) {
            let at = random.below(lines.len() + 1);
            let fresh = |text: String| Line {
                origin: None,
                text,
                ending: ENDINGS[endings],
            };
            match random.below(5) {
                0 => lines.insert(at, fresh(format!("  new{edit} <= 0;"))),
                1 if at < lines.len() => {
                    lines.remove(at);
                }
                2 if at < lines.len() => {
                    // An edit inside a line.
                    let line = &mut lines[at];
                    let characters: Vec<char> = line.text.chars().collect();
                    let from = random.below(characters.len() + 1);
                    let to = from + random.below(characters.len() - from + 1);
                    let replaced = if random.below(2) == 0 { "" } else { "Zé9" };
                    line.text = characters[..from]
                        .iter()
                        .copied()
                        .chain(replaced.chars())
                        .chain(characters[to..].iter().copied())
                        .collect();
                    line.origin = None;
                }
                3 if !original.is_empty() => {
                    let text = original[random.below(original.len())].text.clone();
                    lines.insert(at, fresh(text));
                    copied = true;
                }
                4 => {
                    let end = at + random.below(lines.len() - at + 1);
                    lines.drain(at..end);
                }
                _ => {}
            }
        }
        let (analysed, current) = (join(&original), join(&lines));
        let map = PositionMap::new(&analysed, Arc::new(current.clone()));
        assert!(map.is_complete());
        let analysed_lines: Vec<&str> = SourcePositions::new(&analysed).lines().collect();
        let current_lines: Vec<&str> = SourcePositions::new(&current).lines().collect();
        let utf16 = |line: &str| line.encode_utf16().collect::<Vec<u16>>();
        let mut last = None;
        for (number, line) in analysed_lines.iter().enumerate() {
            let units = utf16(line);
            for (column, unit) in units.iter().enumerate() {
                let place = Position::new(number as u32, column as u32);
                let range = Range::new(place, Position::new(place.line, place.character + 1));
                let Some(now) = map.current(range) else {
                    continue;
                };
                let found = utf16(current_lines[now.start.line as usize]);
                assert_eq!(
                    found[now.start.character as usize], *unit,
                    "seed {seed}: {place:?} became {now:?}\n{analysed:?}\n{current:?}"
                );
                assert!(
                    last < Some(now.start),
                    "seed {seed}: {place:?} became {now:?}"
                );
                last = Some(now.start);
                assert_eq!(map.analysed(now.start).next(), Some(place), "seed {seed}");
                assert_eq!(
                    map.line(now.start.line),
                    Some(current_lines[now.start.line as usize])
                );
            }
        }
        let count = |lines: &[&str], line: &str| {
            lines
                .iter()
                .filter(|other| other.trim() == line.trim())
                .count()
        };
        for (now, line) in lines.iter().enumerate() {
            // A copy whose original went is text that moved.
            let Some(origin) = line.origin.filter(|_| !copied) else {
                continue;
            };
            let once = line.text.contains(is_word)
                && count(&analysed_lines, &line.text) == 1
                && count(&current_lines, &line.text) == 1;
            if once {
                let end = utf16_len(&line.text);
                let range = |line: usize| {
                    let line = line as u32;
                    Range::new(Position::new(line, 0), Position::new(line, end))
                };
                assert_eq!(
                    map.current(range(origin)),
                    Some(range(now)),
                    "seed {seed}: line {origin}\n{analysed:?}\n{current:?}\n{map:?}"
                );
            }
        }
    }
}

/// The answers do not depend on how many edits made the text: undone edits
/// leave none.
#[test]
fn an_undone_edit_leaves_every_place() {
    let text = "module m;\n  wire a;\n  wire a;\nendmodule\n";
    let map = PositionMap::new(text, Arc::new(text.to_owned()));
    for (number, line) in text.lines().enumerate() {
        let place = Position::new(number as u32, 0);
        let range = Range::new(place, Position::new(place.line, line.len() as u32));
        assert_eq!(map.current(range), Some(range));
    }
}
