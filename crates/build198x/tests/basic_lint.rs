//! The `basic lint` rules, one test per rule, on the smallest listing that
//! shows each.

use build198x::basic::{
    Machine,
    lint::{check, fix},
};

const ZX: Machine = Machine::SinclairZxSpectrum;
const C64: Machine = Machine::CommodoreC64;

fn rules(m: Machine, src: &str) -> Vec<(usize, &'static str)> {
    check(m, src)
        .expect("listing lexes")
        .into_iter()
        .map(|f| (f.line, f.rule))
        .collect()
}

#[test]
fn listing_form_flags_what_list_would_change() {
    assert_eq!(rules(ZX, "10 PRINT CHR$(147)\n"), vec![(1, "listing-form")]);
    assert!(rules(ZX, "  10 PRINT CHR$ (147)\n").is_empty());
}

#[test]
fn stored_spaces_outside_strings_are_flagged() {
    // LIST shows these exactly as typed, so listing-form cannot see them.
    assert_eq!(
        rules(ZX, "  10 LET n = n + 1\n"),
        vec![(1, "stored-space"); 4]
    );
    assert!(rules(ZX, "  10 PRINT \"a = b\": REM x = y\n").is_empty());
    // A space inside a numeric variable name is part of the name the ROM allows.
    assert!(rules(ZX, "  10 LET my score=0\n").is_empty());
}

#[test]
fn stored_space_columns_point_at_the_space() {
    let found = check(ZX, "  10 LET n = n + 1\n").expect("lexes");
    let columns: Vec<usize> = found.iter().map(|f| f.column).collect();
    assert_eq!(columns, vec![11, 13, 15, 17]);
}

#[test]
fn fix_rewrites_to_the_listed_form_and_is_idempotent() {
    let fixed = fix(ZX, "10 PRINT CHR$(147)\r\n20 IF a = 1 THEN STOP\n")
        .expect("lexes")
        .expect("changes");
    assert_eq!(fixed, "  10 PRINT CHR$ (147)\n  20 IF a=1 THEN STOP\n");
    assert_eq!(fix(ZX, &fixed).expect("lexes"), None);
    assert!(rules(ZX, &fixed).is_empty());
}

#[test]
fn fix_drops_blank_lines_and_keeps_the_spaces_inside_names() {
    // LIST never shows a blank line, so `fix` removes it rather than keeping
    // it; the spaces inside a numeric variable's name are untouched.
    let fixed = fix(ZX, "10 LET my score = 0\n\n20 STOP\n")
        .expect("lexes")
        .expect("changes");
    assert_eq!(fixed, "  10 LET my score=0\n  20 STOP\n");
}

#[test]
fn blank_lines_are_flagged_as_listing_form() {
    assert_eq!(
        rules(ZX, "  10 STOP\n\n  20 STOP\n"),
        vec![(2, "listing-form")]
    );
    assert!(rules(ZX, "  10 STOP\n  20 STOP\n").is_empty());
}

#[test]
fn fix_leaves_a_canonical_no_blank_line_listing_byte_identical() {
    // A file with no blank lines that is otherwise canonical must not be
    // rewritten at all, so a Makefile does not touch its mtime for nothing.
    assert_eq!(fix(ZX, "  10 STOP\n  20 STOP\n").expect("lexes"), None);
    assert_eq!(fix(C64, "10 END\n").expect("lexes"), None);
}

#[test]
fn string_var_names_must_be_one_letter() {
    assert_eq!(
        rules(ZX, "  10 LET name$=\"x\"\n"),
        vec![(1, "string-var-name")]
    );
    assert!(rules(ZX, "  10 LET n$=\"name$\": PRINT CHR$ 65\n").is_empty());
}

#[test]
fn keyword_named_variables_are_flagged() {
    assert_eq!(rules(ZX, "  10 LET ink=2\n"), vec![(1, "keyword-var-name")]);
    assert!(rules(ZX, "  10 LET inky=2\n").is_empty());
}

/// Each row of the ROM capture: whether RUN stopped with report C, and the
/// line. See tests/fixtures/rom-statement-keyword/README.md.
fn rom_statement_cases() -> Vec<(bool, &'static str)> {
    include_str!("fixtures/rom-statement-keyword/results.tsv")
        .lines()
        .filter(|row| !row.starts_with('#'))
        .map(|row| {
            let cols: Vec<&str> = row.split('\t').collect();
            assert_eq!(cols.len(), 3, "{row}");
            let (editor, run, source) = (cols[0], cols[1], cols[2]);
            let nonsense = run.starts_with("C Nonsense in BASIC");
            // The editor refuses only what RUN also stops on.
            assert!(editor == "accepted" || nonsense, "{row}");
            (nonsense, source)
        })
        .collect()
}

#[test]
fn statement_keyword_flags_exactly_what_the_rom_refuses() {
    let cases = rom_statement_cases();
    assert!(cases.iter().any(|&(nonsense, _)| nonsense));
    assert!(cases.iter().any(|&(nonsense, _)| !nonsense));
    for (nonsense, source) in cases {
        let src = format!("{source}\n");
        let found = rules(ZX, &src);
        let flagged = found.iter().any(|&(_, rule)| rule == "statement-keyword");
        assert_eq!(flagged, nonsense, "{source}: {found:?}");
    }
}

#[test]
fn statement_keyword_points_at_the_statement_and_hints() {
    let found = check(ZX, "  10 PRINT 1:GOTO 10\n").expect("lexes");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].column, 14);
    assert!(found[0].message.contains("`GO TO`"), "{}", found[0].message);
    let found = check(ZX, "  10x=1\n").expect("lexes");
    assert!(
        found[0].message.contains("needs `LET`"),
        "{}",
        found[0].message
    );
}

#[test]
fn fix_leaves_a_line_the_rom_refuses_as_written() {
    // Its listed form, `  20GOTO 10`, is refused too, so fix has nothing to
    // mend there; the other lines are still rewritten.
    let fixed = fix(ZX, "10 PRINT CHR$(147)\n20 GOTO 10\n")
        .expect("lexes")
        .expect("changes");
    assert_eq!(fixed, "  10 PRINT CHR$ (147)\n20 GOTO 10\n");
    assert_eq!(rules(ZX, &fixed), vec![(2, "statement-keyword")]);
    assert_eq!(fix(ZX, &fixed).expect("lexes"), None);
}

#[test]
fn c64_two_letter_clash() {
    // Names with no keyword inside them: SCORE would tokenise its OR.
    let found = rules(C64, "10 SPEED=1\n20 SPIN=2\n30 SP$=\"A\"\n");
    assert_eq!(found, vec![(2, "var-name-clash")]);
    // An array is kept apart from the plain variable of the same name, and
    // `%` makes an integer variable.
    assert!(rules(C64, "10 SPEED=1\n20 SPIN(2)=2\n30 SP%=3\n").is_empty());
}

#[test]
fn c64_lowercase_lists_as_uppercase() {
    // The C64 stores typed lowercase ASCII as the uppercase PETSCII letter,
    // so LIST shows `10 PRINT "LOWER"`: the listed form differs, and no
    // variable rule fires on a keyword typed in lowercase.
    assert_eq!(
        rules(C64, "10 print \"lower\"\n"),
        vec![(1, "listing-form")]
    );
    assert!(rules(C64, "10 PRINT \"LOWER\"\n").is_empty());
}

#[test]
fn c64_data_values_are_not_variables() {
    assert!(rules(C64, "10 DATA SPEED,SPIN\n20 READ A$\n").is_empty());
}

#[test]
fn c64_keyword_inside_a_name() {
    // SCORE is stored as S C <OR> E; the C64 answers ?SYNTAX ERROR.
    assert_eq!(rules(C64, "10 SCORE=1\n"), vec![(1, "keyword-in-name")]);
    assert_eq!(
        rules(C64, "10 PRINT A+SCORE\n"),
        vec![(1, "keyword-in-name")]
    );
    // One-sided contact is ordinary unspaced BASIC V2 and is not flagged.
    assert!(rules(C64, "10 FORI=1TO10:PRINTA:NEXT\n").is_empty());
    assert!(rules(C64, "10 FOR I=1 TO 10:NEXT\n").is_empty());
    assert!(rules(C64, "10 PRINT \"SCORE\"\n").is_empty());
    // Operators are keyword tokens too, but a name either side is ordinary.
    assert!(rules(C64, "10 A=B+C\n").is_empty());
}

#[test]
fn c64_keyword_in_name_message_offers_the_spaced_reading() {
    let found = check(C64, "10 SCORE=1\n").expect("lexes");
    assert_eq!(found[0].column, 4);
    assert!(
        found[0].message.contains("`SCORE` has the keyword OR"),
        "{}",
        found[0].message
    );
    assert!(found[0].message.contains("SC OR E"), "{}", found[0].message);
}

#[test]
fn line_order() {
    assert_eq!(
        rules(ZX, "  20 STOP\n  10 STOP\n  10 STOP\n"),
        vec![(2, "line-order"), (3, "line-order")]
    );
}

#[test]
fn unreadable_lines_are_errors_naming_the_line() {
    let err = check(ZX, "  10 STOP\nSTOP\n").expect_err("no line number");
    assert_eq!(err.line, 2);
    let err = check(C64, "10 END\n20 PRINT \"\u{e9}\"\n").expect_err("not ASCII");
    assert_eq!(err.line, 2);
}
