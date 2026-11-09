use super::append_lines;

#[test]
fn fonts_reset_each_line_and_trailing_blank_lines_are_retained() {
    let mut lines = Vec::new();
    append_lines(&mut lines, "[font0]Heading\\nPerson\\n[font1]Other\\n\\n").unwrap();
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0].text, "Heading");
    assert!(lines[0].heading);
    assert_eq!(lines[1].text, "Person");
    assert!(!lines[1].heading);
    assert!(!lines[2].heading);
    assert!(lines[3].text.is_empty());
    assert!(lines[4].text.is_empty());
}

#[test]
fn literal_delimiter_and_two_ordered_prefix_tests_match_the_source() {
    let mut lines = Vec::new();
    append_lines(
        &mut lines,
        "[font0][font1]Mixed\\nKeep\nunit\\n[font2]Untouched",
    )
    .unwrap();
    assert_eq!(lines.len(), 3);
    assert!(!lines[0].heading);
    assert_eq!(lines[0].text, "Mixed");
    assert_eq!(lines[1].text, "Keep\nunit");
    assert_eq!(lines[2].text, "[font2]Untouched");
}

#[test]
fn oversized_credit_lines_and_rolls_fail_explicitly() {
    assert!(append_lines(&mut Vec::new(), &"x".repeat(1_025)).is_err());
    assert!(append_lines(&mut Vec::new(), &"\\n".repeat(4_096)).is_err());
}
