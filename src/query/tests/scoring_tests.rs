use super::*;
use regex::Regex;

#[test]
fn test_score_match_empty_query() {
    let score = score_match("", "any_name", "any_display_fqn", "any_fqn", None);
    assert_eq!(score, 80, "Empty query matches via name.starts_with('')");
}

#[test]
fn test_score_match_exact_name() {
    let score = score_match("foo", "foo", "", "", None);
    assert_eq!(score, 100, "Exact name match should return score 100");
}

#[test]
fn test_score_match_exact_display_fqn() {
    let score = score_match("foo", "", "foo", "", None);
    assert_eq!(score, 95, "Exact display_fqn match should return score 95");
}

#[test]
fn test_score_match_exact_fqn() {
    let score = score_match("foo", "", "", "foo", None);
    assert_eq!(score, 90, "Exact fqn match should return score 90");
}

#[test]
fn test_score_match_name_prefix() {
    let score = score_match("foo", "foobar", "", "", None);
    assert_eq!(score, 80, "Name prefix match should return score 80");
}

#[test]
fn test_score_match_display_fqn_prefix() {
    let score = score_match("foo", "", "foobar", "", None);
    assert_eq!(score, 70, "Display_fqn prefix match should return score 70");
}

#[test]
fn test_score_match_name_contains() {
    let score = score_match("foo", "barfoobar", "", "", None);
    assert_eq!(score, 60, "Name contains match should return score 60");
}

#[test]
fn test_score_match_display_fqn_contains() {
    let score = score_match("foo", "", "barfoobar", "", None);
    assert_eq!(
        score, 50,
        "Display_fqn contains match should return score 50"
    );
}

#[test]
fn test_score_match_fqn_contains() {
    let score = score_match("foo", "", "", "barfoobar", None);
    assert_eq!(score, 40, "Fqn contains match should return score 40");
}

#[test]
fn test_score_match_tie_handling() {
    let score1 = score_match("test", "test_value", "", "", None);
    let score2 = score_match("test", "test_another", "", "", None);
    assert_eq!(
        score1, score2,
        "Equivalent matches should produce equal scores"
    );
}

#[test]
fn test_score_match_regex_name() {
    let regex = Regex::new("foo.*").ok();
    let score = score_match("foo.*", "foobar", "", "", regex.as_ref());
    assert_eq!(score, 70, "Regex match on name should return score 70");
}

#[test]
fn test_score_match_regex_display_fqn() {
    let regex = Regex::new("foo.*").ok();
    let score = score_match("foo.*", "", "foobar", "", regex.as_ref());
    assert_eq!(
        score, 60,
        "Regex match on display_fqn should return score 60"
    );
}

#[test]
fn test_score_match_regex_fqn() {
    let regex = Regex::new("foo.*").ok();
    let score = score_match("foo.*", "", "", "foobar", regex.as_ref());
    assert_eq!(score, 50, "Regex match on fqn should return score 50");
}

#[test]
fn test_score_match_boundary_max() {
    let score = score_match("test", "test", "test", "test", None);
    assert_eq!(score, 100, "Score should never exceed 100");
}

#[test]
fn test_score_match_no_match() {
    let score = score_match("xyz", "abc", "def", "ghi", None);
    assert_eq!(score, 0, "No match should return score 0");
}

#[test]
fn test_score_match_regex_no_match() {
    let regex = Regex::new("xyz.*").ok();
    let score = score_match("xyz.*", "abc", "def", "ghi", regex.as_ref());
    assert_eq!(score, 0, "Regex no match should return score 0");
}

#[test]
fn test_score_match_priority_exact_over_prefix() {
    let score = score_match("foo", "foo", "foobar", "", None);
    assert_eq!(
        score, 100,
        "Exact name match should take priority over prefix"
    );
}

#[test]
fn test_score_match_priority_prefix_over_contains() {
    let score = score_match("foo", "foobar", "barfoobar", "", None);
    assert_eq!(score, 80, "Prefix match should take priority over contains");
}

#[test]
fn test_score_match_multiple_matches_highest_score() {
    let score = score_match("foo", "foo", "foobar", "barfoobar", None);
    assert_eq!(score, 100, "Should return highest score from all matches");
}

#[test]
fn test_score_match_case_sensitive() {
    let score1 = score_match("foo", "foo", "", "", None);
    let score2 = score_match("foo", "Foo", "", "", None);
    assert_eq!(score1, 100, "Exact case match should return 100");
    assert_eq!(score2, 0, "Different case should not match");
}

#[test]
fn test_score_match_empty_name_field() {
    let score = score_match("foo", "", "", "", None);
    assert_eq!(
        score, 0,
        "All empty fields with non-empty query should return 0"
    );
}
