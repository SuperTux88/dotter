use anyhow::{Context, Result};
use crossterm::style::Colorize;
use diff;
use handlebars::Handlebars;

use std::cmp::{max, min};
use std::fs;

use config::Variables;
use file_state;

pub type Diff = Vec<diff::Result<String>>;
pub type HunkDiff = Vec<(usize, usize, Diff)>;

pub fn generate_diff(
    template: &file_state::TemplateDescription,
    handlebars: &Handlebars,
    variables: &Variables,
) -> Result<Diff> {
    let file_contents =
        fs::read_to_string(&template.source).context("read template source file")?;
    let file_contents = template.apply_actions(file_contents);
    let rendered = handlebars
        .render_template(&file_contents, variables)
        .context("render template")?;

    let target_contents =
        fs::read_to_string(&template.target.target).context("read template target file")?;

    let diff_result = diff::lines(&target_contents, &rendered);

    Ok(diff_result.into_iter().map(to_owned_diff_result).collect())
}

fn to_owned_diff_result(from: diff::Result<&str>) -> diff::Result<String> {
    match from {
        diff::Result::Left(s) => diff::Result::Left(s.to_string()),
        diff::Result::Right(s) => diff::Result::Right(s.to_string()),
        diff::Result::Both(s1, s2) => diff::Result::Both(s1.to_string(), s2.to_string()),
    }
}

pub fn diff_nonempty(diff: &[diff::Result<String>]) -> bool {
    for line in diff {
        match line {
            diff::Result::Both(..) => {}
            _ => {
                return true;
            }
        }
    }
    false
}

fn hunkify_diff(diff: Diff, extra_lines: usize) -> HunkDiff {
    let mut hunks = vec![];

    let mut left_line_number: usize = 0;
    let mut right_line_number: usize = 0;

    let mut current_hunk = None;

    for position in 0..diff.len() {
        let line = &diff[position];
        match line {
            diff::Result::Left(_) => {
                left_line_number += 1;
                if current_hunk.is_none() {
                    current_hunk = Some((left_line_number, right_line_number, vec![]));
                }
                current_hunk.as_mut().unwrap().2.push(line.clone());
            }
            diff::Result::Right(_) => {
                right_line_number += 1;
                if current_hunk.is_none() {
                    current_hunk = Some((left_line_number, right_line_number, vec![]));
                }
                current_hunk.as_mut().unwrap().2.push(line.clone());
            }
            diff::Result::Both(_, _) => {
                left_line_number += 1;
                right_line_number += 1;

                if diff[position..=min(position + extra_lines, diff.len() - 1)]
                    .iter()
                    .any(is_different)
                {
                    if current_hunk.is_none() {
                        current_hunk = Some((left_line_number, right_line_number, vec![]));
                    }
                    current_hunk.as_mut().unwrap().2.push(line.clone());
                } else if diff[position.saturating_sub(extra_lines)..position]
                    .iter()
                    .any(is_different)
                {
                    current_hunk.as_mut().unwrap().2.push(line.clone());
                } else if let Some(hunk) = current_hunk.take() {
                    hunks.push(hunk);
                }
            }
        }
    }

    if let Some(hunk) = current_hunk {
        hunks.push(hunk);
    }

    hunks
}

fn is_different(diff: &diff::Result<String>) -> bool {
    !matches!(diff, diff::Result::Both(..))
}

fn print_hunk(mut left_line: usize, mut right_line: usize, hunk: Diff, max_digits: usize) {
    for line in hunk {
        match line {
            diff::Result::Left(l) => {
                left_line += 1;
                println!(
                    " {:>width$} | {:>width$} | {}",
                    left_line.to_string().red(),
                    "",
                    l.red(),
                    width = max_digits
                );
            }
            diff::Result::Both(l, _) => {
                left_line += 1;
                right_line += 1;
                println!(
                    " {:>width$} | {:>width$} | {}",
                    left_line.to_string().dark_grey(),
                    right_line.to_string().dark_grey(),
                    l,
                    width = max_digits
                );
            }
            diff::Result::Right(r) => {
                right_line += 1;
                println!(
                    " {:>width$} | {:>width$} | {}",
                    "",
                    right_line.to_string().green(),
                    r.green(),
                    width = max_digits
                );
            }
        }
    }
}

pub fn print_diff(diff: Diff, extra_lines: usize) {
    let mut diff = hunkify_diff(diff, extra_lines);

    let last_hunk = diff.pop().expect("at least one hunk");
    let max_possible_line = max(last_hunk.0, last_hunk.1) + last_hunk.2.len();
    let max_possible_digits = max_possible_line.to_string().len(); // yes I could log10, whatever

    for hunk in diff {
        print_hunk(hunk.0, hunk.1, hunk.2, max_possible_digits);
        println!();
    }

    print_hunk(last_hunk.0, last_hunk.1, last_hunk.2, max_possible_digits);
}
