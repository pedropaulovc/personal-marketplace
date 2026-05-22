//! PreToolUse hook that enforces `--headed` on `playwright-cli open` invocations
//! in Bash and PowerShell commands.
//!
//! Rule: any `playwright-cli ... open ...` invocation must include `--headed`.
//! When missing, the hook auto-injects ` --headed` immediately after the `open`
//! subcommand token, leaving the rest of the command byte-for-byte unchanged.
//!
//! Returns JSON with `updatedInput` so Claude Code executes the corrected
//! command transparently. Claude can bypass rewriting by adding `[no-rewrite]`
//! to the tool description.

use serde_json::{json, Value};
use std::io::{self, Read};
use std::process;

fn main() {
    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        process::exit(0);
    }

    let data: Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => process::exit(0),
    };

    let tool_name = data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
    if !matches!(tool_name, "Bash" | "PowerShell") {
        process::exit(0);
    }

    let tool_input = match data.get("tool_input") {
        Some(v) => v,
        None => process::exit(0),
    };

    let description = tool_input
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if description.contains("[no-rewrite]") {
        process::exit(0);
    }

    let command = match tool_input.get("command").and_then(|v| v.as_str()) {
        Some(c) if !c.is_empty() => c,
        _ => process::exit(0),
    };

    if let Some(fixed) = fix_command(command) {
        let mut updated = tool_input.as_object().cloned().unwrap_or_default();
        updated.insert("command".into(), Value::String(fixed.command));

        let output = json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "updatedInput": updated,
                "additionalContext": fixed.context
            }
        });
        println!("{}", output);
    }

    process::exit(0);
}

// ---------------------------------------------------------------------------
// Fix orchestrator
// ---------------------------------------------------------------------------

struct FixResult {
    command: String,
    context: String,
}

const NEEDLE: &[u8] = b"playwright-cli";

/// Apply the fix across every `playwright-cli` invocation in the command.
/// Returns `Some(FixResult)` if any invocation was rewritten.
fn fix_command(command: &str) -> Option<FixResult> {
    let bytes = command.as_bytes();
    let mut result = String::with_capacity(command.len() + 16);
    let mut cursor = 0;
    let mut rewrites = 0;
    let mut i = 0;

    while i + NEEDLE.len() <= bytes.len() {
        if !is_word_start(bytes, i) {
            i += 1;
            continue;
        }
        if &bytes[i..i + NEEDLE.len()] != NEEDLE {
            i += 1;
            continue;
        }
        let after = i + NEEDLE.len();
        if !is_word_end(bytes, after) {
            i += 1;
            continue;
        }

        // Found a playwright-cli invocation at byte i. Determine where its arg
        // list ends — the next unquoted shell separator (`;`, `|`, `&`, newline)
        // or end-of-string.
        let args_end = find_args_end(bytes, after);
        let tokens = tokenize(&command[after..args_end], after);

        let has_headed = tokens.iter().any(|t| {
            t.text == "--headed" || t.text.starts_with("--headed=") || t.text == "-headed"
        });
        let open_token = tokens.iter().find(|t| t.text == "open");

        if !has_headed {
            if let Some(open) = open_token {
                result.push_str(&command[cursor..open.end]);
                result.push_str(" --headed");
                cursor = open.end;
                rewrites += 1;
            }
        }

        i = args_end;
    }

    if rewrites == 0 {
        return None;
    }

    result.push_str(&command[cursor..]);

    let context = format!(
        "playwright-cli-headed hook rewrote this command: added `--headed` to {} `playwright-cli open` invocation{} (rule: playwright-cli open must always run --headed so the browser is visible). To bypass rewriting, add [no-rewrite] to the tool description.",
        rewrites,
        if rewrites == 1 { "" } else { "s" }
    );

    Some(FixResult { command: result, context })
}

// ---------------------------------------------------------------------------
// Word boundaries
// ---------------------------------------------------------------------------

/// `playwright-cli` is a fresh token start when the preceding byte is whitespace,
/// a path separator (so `/usr/local/bin/playwright-cli` matches), a quote, or a
/// shell command opener (`;`, `|`, `&`, `(`, `=`, backtick, `$`).
fn is_word_start(bytes: &[u8], i: usize) -> bool {
    if i == 0 {
        return true;
    }
    matches!(
        bytes[i - 1],
        b' ' | b'\t'
            | b'\n'
            | b'\r'
            | b'/'
            | b'\\'
            | b'"'
            | b'\''
            | b'`'
            | b';'
            | b'&'
            | b'|'
            | b'('
            | b'='
            | b'$'
    )
}

/// End-of-token: either at end-of-string or the next byte is whitespace/separator.
fn is_word_end(bytes: &[u8], i: usize) -> bool {
    if i >= bytes.len() {
        return true;
    }
    matches!(
        bytes[i],
        b' ' | b'\t' | b'\n' | b'\r' | b';' | b'|' | b'&' | b')' | b'\'' | b'"' | b'`'
    )
}

// ---------------------------------------------------------------------------
// Argument-slice detection
// ---------------------------------------------------------------------------

/// Walk forward from `start` until we hit an unquoted shell statement separator
/// (`;`, `&`, `|`, newline) or end-of-string. Quoted strings are skipped so a
/// `;` inside `'foo;bar'` doesn't terminate the slice.
fn find_args_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\'' || b == b'"' {
            i = skip_quoted(bytes, i);
            continue;
        }
        if matches!(b, b';' | b'&' | b'|' | b'\n') {
            return i;
        }
        i += 1;
    }
    bytes.len()
}

/// Given `bytes[i]` is a quote, return the index just past the matching closing
/// quote (or end of string if unterminated).
fn skip_quoted(bytes: &[u8], i: usize) -> usize {
    let quote = bytes[i];
    let mut j = i + 1;
    while j < bytes.len() {
        let b = bytes[j];
        if b == b'\\' && quote == b'"' && j + 1 < bytes.len() {
            j += 2;
            continue;
        }
        if b == quote {
            return j + 1;
        }
        j += 1;
    }
    bytes.len()
}

// ---------------------------------------------------------------------------
// Tokenization
// ---------------------------------------------------------------------------

struct Token {
    /// Token text with any surrounding quotes stripped.
    text: String,
    /// Byte offset of the token's last byte + 1 in the original command string.
    end: usize,
}

/// Tokenize a slice of shell arguments. `base` is the byte offset of `slice`
/// inside the original command; recorded `end` positions are absolute.
fn tokenize(slice: &str, base: usize) -> Vec<Token> {
    let bytes = slice.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        // Skip leading whitespace
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }

        let start = i;
        let mut text = String::new();

        // Consume the token, which may interleave quoted and unquoted runs
        while i < bytes.len() {
            let b = bytes[i];
            if matches!(b, b' ' | b'\t' | b'\r' | b'\n') {
                break;
            }
            if b == b'\'' || b == b'"' {
                let close = skip_quoted(bytes, i);
                let inner_end = if close <= bytes.len() && close > i + 1
                    && bytes.get(close - 1).copied() == Some(b)
                {
                    close - 1
                } else {
                    close
                };
                text.push_str(&slice[i + 1..inner_end]);
                i = close;
                continue;
            }
            text.push(b as char);
            i += 1;
        }

        tokens.push(Token {
            text,
            end: base + i,
        });

        let _ = start; // suppress unused warning under some configurations
    }

    tokens
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(cmd: &str) -> String {
        fix_command(cmd).unwrap().command
    }

    // -- Basic insertion -----------------------------------------------------

    #[test]
    fn adds_headed_after_open() {
        assert_eq!(
            fixed("playwright-cli open https://example.com"),
            "playwright-cli open --headed https://example.com"
        );
    }

    #[test]
    fn adds_headed_with_no_extra_args() {
        assert_eq!(fixed("playwright-cli open"), "playwright-cli open --headed");
    }

    #[test]
    fn adds_headed_with_flags_before_open() {
        assert_eq!(
            fixed("playwright-cli --verbose open https://example.com"),
            "playwright-cli --verbose open --headed https://example.com"
        );
    }

    #[test]
    fn adds_headed_with_flags_after_open() {
        assert_eq!(
            fixed("playwright-cli open --device 'iPhone 15' https://example.com"),
            "playwright-cli open --headed --device 'iPhone 15' https://example.com"
        );
    }

    // -- No-op cases ---------------------------------------------------------

    #[test]
    fn skips_when_headed_already_present() {
        assert!(fix_command("playwright-cli open --headed https://example.com").is_none());
    }

    #[test]
    fn skips_when_headed_appears_before_open() {
        assert!(fix_command("playwright-cli --headed open https://example.com").is_none());
    }

    #[test]
    fn skips_when_headed_equals_form() {
        assert!(fix_command("playwright-cli open --headed=true https://example.com").is_none());
    }

    #[test]
    fn skips_when_no_open_subcommand() {
        assert!(fix_command("playwright-cli codegen https://example.com").is_none());
    }

    #[test]
    fn skips_when_not_playwright_cli() {
        assert!(fix_command("playwright open https://example.com").is_none());
    }

    #[test]
    fn skips_when_playwright_cli_is_substring_of_another_word() {
        // foo-playwright-cli-bar should not match
        assert!(fix_command("foo-playwright-cli-bar open https://example.com").is_none());
    }

    // -- Path & shell-context invocations ------------------------------------

    #[test]
    fn handles_full_path_invocation() {
        assert_eq!(
            fixed("/usr/local/bin/playwright-cli open https://example.com"),
            "/usr/local/bin/playwright-cli open --headed https://example.com"
        );
    }

    #[test]
    fn handles_relative_path_invocation() {
        assert_eq!(
            fixed("./playwright-cli open https://example.com"),
            "./playwright-cli open --headed https://example.com"
        );
    }

    #[test]
    fn handles_env_prefix() {
        assert_eq!(
            fixed("DEBUG=1 playwright-cli open https://example.com"),
            "DEBUG=1 playwright-cli open --headed https://example.com"
        );
    }

    // -- Multi-statement commands --------------------------------------------

    #[test]
    fn rewrites_only_the_offending_statement() {
        assert_eq!(
            fixed("echo hi && playwright-cli open https://example.com"),
            "echo hi && playwright-cli open --headed https://example.com"
        );
    }

    #[test]
    fn does_not_pull_open_from_a_different_statement() {
        // The bare `open` in the second statement must not satisfy the first
        // playwright-cli invocation (which only has `codegen` as its subcommand).
        assert!(fix_command("playwright-cli codegen url; open file.txt").is_none());
    }

    #[test]
    fn rewrites_multiple_invocations() {
        assert_eq!(
            fixed("playwright-cli open a; playwright-cli open b"),
            "playwright-cli open --headed a; playwright-cli open --headed b"
        );
    }

    // -- PowerShell-style separators -----------------------------------------

    #[test]
    fn handles_powershell_semicolon_chain() {
        assert_eq!(
            fixed("Write-Host hi; playwright-cli open https://example.com"),
            "Write-Host hi; playwright-cli open --headed https://example.com"
        );
    }

    // -- Quoting edge cases --------------------------------------------------

    #[test]
    fn ignores_open_appearing_inside_a_quoted_url() {
        // The url 'https://example.com/open' is a single token; "open" should
        // not be recognized as the subcommand. Without an `open` subcommand, no
        // rewrite happens.
        assert!(fix_command("playwright-cli codegen 'https://example.com/open'").is_none());
    }

    #[test]
    fn quoted_open_argument_still_counts_as_subcommand() {
        // 'open' as a quoted single token is still the literal string `open`.
        assert_eq!(
            fixed("playwright-cli 'open' https://example.com"),
            "playwright-cli 'open' --headed https://example.com"
        );
    }
}
