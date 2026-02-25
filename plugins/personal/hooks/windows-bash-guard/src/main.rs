//! PreToolUse hook that detects common Windows+bash path pitfalls in Bash
//! commands before execution, preventing cryptic runtime errors.
//!
//! Detects:
//! 1. `/dev/stdin` (stdout/stderr) in node commands — doesn't exist on Windows
//! 2. Windows backslash paths in `node -e` inline code — JS escape sequences
//!    like `\t` (tab), `\n` (newline), `\b` (backspace) silently corrupt paths
//! 3. Unquoted Windows backslash paths in bash — backslash is an escape char,
//!    so `ls C:\src` becomes `ls C:src`
//! 4. Trailing `\"` in double-quoted Windows paths — `"C:\path\"` eats the
//!    closing quote, causing `unexpected EOF`

use serde_json::Value;
use std::io::{self, Read};
use std::process;

fn main() {
    if std::env::consts::OS != "windows" {
        process::exit(0);
    }

    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        process::exit(0);
    }

    let data: Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => process::exit(0),
    };

    if data.get("tool_name").and_then(|v| v.as_str()) != Some("Bash") {
        process::exit(0);
    }

    let command = match data
        .get("tool_input")
        .and_then(|v| v.get("command"))
        .and_then(|v| v.as_str())
    {
        Some(c) if !c.is_empty() => c,
        _ => process::exit(0),
    };

    // Check each pattern; first match wins
    let checks: &[fn(&str) -> Option<String>] = &[
        check_dev_stdin,
        check_trailing_backslash_quote,
        check_node_eval_backslash_paths,
        check_unquoted_backslash_paths,
    ];

    for check in checks {
        if let Some(msg) = check(command) {
            block(&msg);
        }
    }

    process::exit(0);
}

fn block(reason: &str) -> ! {
    eprint!("{}", reason);
    process::exit(2);
}

// ---------------------------------------------------------------------------
// Pattern 1: /dev/stdin in node commands
// ---------------------------------------------------------------------------

/// Detect `/dev/stdin`, `/dev/stdout`, `/dev/stderr` in commands that invoke node.
/// These paths don't exist on Windows — Node resolves them as `C:\dev\stdin`.
fn check_dev_stdin(command: &str) -> Option<String> {
    if !command.contains("node") {
        return None;
    }

    for dev_path in &["/dev/stdin", "/dev/stdout", "/dev/stderr"] {
        if !command.contains(dev_path) {
            continue;
        }

        return Some(format!(
            "BLOCKED: {dev_path} does not exist on Windows (Node resolves it to C:\\dev\\stdin).\n\
             \n\
             Fix options:\n\
             \n\
             1. Write to a temp file first, then pass the path as an argument:\n\
             \n\
                some_command > \"$TEMP/data.json\"\n\
                node -e \"const d = JSON.parse(require('fs').readFileSync(process.argv[1],'utf8')); ...\" \"%TEMP%\\data.json\"\n\
             \n\
             2. Use process.stdin in the node script:\n\
             \n\
                some_command | node -e \"let b=''; process.stdin.on('data',c=>b+=c); process.stdin.on('end',()=>{{ ... }});\""
        ));
    }

    None
}

// ---------------------------------------------------------------------------
// Pattern 2: Trailing backslash-quote in double-quoted Windows paths
// ---------------------------------------------------------------------------

/// Detect `"C:\some\path\"` where the trailing `\"` escapes the closing quote
/// in bash, causing `unexpected EOF while looking for matching '"'`.
///
/// Real examples from transcripts:
///   ls -la "C:\src\el400\main\.github\workflows\"
///   grep -r "pattern" "C:\src\codjiflo\C\src\styles\" --include="*.css"
fn check_trailing_backslash_quote(command: &str) -> Option<String> {
    let bytes = command.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // Find opening double-quote
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }

        i += 1; // skip past opening quote
        let mut has_drive_path = false;

        while i < bytes.len() {
            let b = bytes[i];

            // Check for drive letter pattern: X:\ inside the quoted string
            if !has_drive_path
                && i + 2 < bytes.len()
                && b.is_ascii_alphabetic()
                && bytes[i + 1] == b':'
                && bytes[i + 2] == b'\\'
            {
                has_drive_path = true;
            }

            // Found \" — if this quoted region contains a drive path, this
            // is almost certainly a path separator being misread as an
            // escaped quote. Check if what follows looks like it should be
            // outside the string (space, operator, or end of command).
            if b == b'\\' && i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                if has_drive_path {
                    let after = bytes.get(i + 2);
                    let looks_like_intended_end = match after {
                        None => true,
                        Some(b' ' | b'\t' | b'\n' | b';' | b'|' | b'&' | b'>') => true,
                        _ => false,
                    };
                    if looks_like_intended_end {
                        return Some(
                            "BLOCKED: Trailing backslash before closing double-quote eats the quote.\n\
                             \n\
                             In bash, \\\" inside double quotes is an escaped literal quote, not a\n\
                             path separator + closing quote. This causes:\n\
                             \n\
                                 unexpected EOF while looking for matching `\"'\n\
                             \n\
                             Fix: Use forward slashes (always work on Windows in bash):\n\
                             \n\
                                 ls -la \"C:/src/project/folder/\"     (works)\n\
                                 ls -la \"C:\\src\\project\\folder\\\"   (broken: \\\" eats the quote)\n\
                             \n\
                             Or drop the trailing slash:\n\
                             \n\
                                 ls -la \"C:\\src\\project\\folder\""
                                .to_string(),
                        );
                    }
                }
                // Treat as escaped quote, skip both chars
                i += 2;
                continue;
            }

            // Unescaped closing quote — end of this string
            if b == b'"' {
                break;
            }

            i += 1;
        }

        i += 1; // skip past closing quote (or move past EOF)
    }

    None
}

// ---------------------------------------------------------------------------
// Pattern 3: Backslash paths in node -e
// ---------------------------------------------------------------------------

/// Detect Windows drive paths with backslashes inside `node -e` / `node --eval`.
///
/// After bash double-quote processing, `\\` becomes `\`. JavaScript then interprets
/// `\t` as tab, `\n` as newline, `\b` as backspace, `\r` as carriage return, etc.
/// This silently corrupts paths like `C:\tmp\file.json` → `C:<TAB>mp<NUL>ile.json`.
///
/// The fix is to use forward slashes: `C:/tmp/file.json` (works on Windows in Node).
fn check_node_eval_backslash_paths(command: &str) -> Option<String> {
    let node_e_pos = find_node_eval_pos(command)?;
    let after_node_e = &command[node_e_pos..];

    // Look for drive-letter paths with backslashes: C:\, D:\, etc.
    let bytes = after_node_e.as_bytes();
    for i in 0..bytes.len().saturating_sub(2) {
        if !bytes[i].is_ascii_alphabetic() || bytes[i + 1] != b':' || bytes[i + 2] != b'\\' {
            continue;
        }
        // Confirm it looks like a path (next char after backslash(es) is alphanumeric)
        let rest = &bytes[i + 2..];
        let after_slashes = rest.iter().position(|&b| b != b'\\').unwrap_or(rest.len());
        if after_slashes >= rest.len() || !rest[after_slashes].is_ascii_alphanumeric() {
            continue;
        }
        return Some(
            "BLOCKED: Windows backslash paths in node -e cause JavaScript escape bugs.\n\
             \n\
             After bash processes the command, paths like C:\\tmp\\ reach JavaScript\n\
             as C:\\t mp\\ where \\t is a TAB character. Same for \\n (newline),\n\
             \\r (carriage return), \\b (backspace), etc.\n\
             \n\
             Fix: Use FORWARD SLASHES in all paths inside node -e:\n\
             \n\
                readFileSync('C:/tmp/file.json')       // works on Windows\n\
                readFileSync('C:\\\\tmp\\\\file.json')  // broken: \\t = tab\n\
             \n\
             Or pass the path as a CLI argument:\n\
             \n\
                node -e \"...readFileSync(process.argv[1])...\" \"C:\\tmp\\file.json\""
                .to_string(),
        );
    }

    None
}

/// Find the position of `node -e` or `node --eval` in the command.
fn find_node_eval_pos(command: &str) -> Option<usize> {
    for pattern in &["node -e ", "node -e\"", "node -e'", "node --eval "] {
        if let Some(pos) = command.find(pattern) {
            return Some(pos);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Pattern 4: Unquoted Windows backslash paths in bash commands
// ---------------------------------------------------------------------------

/// Detect bare (unquoted) Windows drive paths like `ls C:\src\project`.
///
/// In bash, an unquoted backslash escapes the next character, so `\s` becomes
/// just `s`, `\p` becomes `p`, etc. The path `C:\src\project` becomes
/// `C:srcproject` — missing all directory separators.
///
/// Real examples from transcripts:
///   ls -la C:\src\codeflow              → C:srccodeflow
///   rm C:\src\codjiflo\main\file.json   → C:srccodjiflomainfle.json
///   tail -50 C:\Users\pedro\...\out     → C:Userspedro...out
fn check_unquoted_backslash_paths(command: &str) -> Option<String> {
    let bytes = command.as_bytes();

    // Walk through the command looking for drive-letter paths outside of quotes
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut prev_backslash = false;
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];

        // Track quoting state
        if b == b'\'' && !in_double_quote && !prev_backslash {
            in_single_quote = !in_single_quote;
            prev_backslash = false;
            i += 1;
            continue;
        }
        if b == b'"' && !in_single_quote && !prev_backslash {
            in_double_quote = !in_double_quote;
            prev_backslash = false;
            i += 1;
            continue;
        }

        prev_backslash = b == b'\\' && !in_single_quote && !prev_backslash;

        // Only check when outside all quotes
        if !in_single_quote && !in_double_quote {
            // Look for: drive letter + colon + single backslash + alpha
            // In the raw command string, a single `\` means bash will eat it.
            // Two `\\` means bash keeps one `\` — that's fine for most commands
            // (though still wrong for node -e, handled by pattern 3).
            //
            // We specifically detect: X:\ followed by alpha, where the `\` is
            // NOT doubled (i.e. not `X:\\`).
            if i + 3 < bytes.len()
                && b.is_ascii_alphabetic()
                && bytes[i + 1] == b':'
                && bytes[i + 2] == b'\\'
                && bytes[i + 3] != b'\\'
                && bytes[i + 3].is_ascii_alphanumeric()
            {
                // Make sure this is preceded by whitespace or start-of-command
                // to avoid matching inside URLs like "http://C:\" or variable
                // assignments
                let preceded_by_separator = i == 0
                    || matches!(
                        bytes[i - 1],
                        b' ' | b'\t' | b'\n' | b'(' | b';' | b'|' | b'&'
                    );

                if preceded_by_separator {
                    return Some(
                        "BLOCKED: Unquoted Windows path — bash will eat the backslashes.\n\
                         \n\
                         In bash, an unquoted backslash escapes the next character:\n\
                         C:\\src\\project becomes C:srcproject (all separators lost).\n\
                         \n\
                         Fix: Use forward slashes (preferred) or quote the path:\n\
                         \n\
                             ls C:/src/project          (forward slashes — always works)\n\
                             ls \"C:\\src\\project\"       (double-quoted — backslashes preserved)\n\
                             ls 'C:\\src\\project'       (single-quoted — backslashes preserved)"
                            .to_string(),
                    );
                }
            }
        }

        i += 1;
    }

    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Pattern 1: /dev/stdin
    #[test]
    fn detects_dev_stdin_in_node_pipe() {
        let cmd = r#"cat data.json | node -e "const d=JSON.parse(require('fs').readFileSync('/dev/stdin','utf8'))""#;
        assert!(check_dev_stdin(cmd).is_some());
    }

    #[test]
    fn ignores_dev_stdin_without_node() {
        let cmd = "cat /dev/stdin";
        assert!(check_dev_stdin(cmd).is_none());
    }

    // Pattern 2: Trailing backslash-quote
    #[test]
    fn detects_trailing_backslash_quote() {
        let cmd = r#"ls -la "C:\src\el400\main\.github\workflows\""#;
        assert!(check_trailing_backslash_quote(cmd).is_some());
    }

    #[test]
    fn detects_trailing_backslash_quote_in_grep() {
        let cmd = r#"grep -r "pattern" "C:\src\codjiflo\C\src\styles\" --include="*.css""#;
        assert!(check_trailing_backslash_quote(cmd).is_some());
    }

    #[test]
    fn allows_properly_quoted_path() {
        let cmd = r#"ls -la "C:\src\project""#;
        assert!(check_trailing_backslash_quote(cmd).is_none());
    }

    // Pattern 3: Backslash paths in node -e
    #[test]
    fn detects_backslash_path_in_node_e() {
        let cmd = r#"node -e "require('fs').readFileSync('C:\\src\\file.json','utf8')""#;
        assert!(check_node_eval_backslash_paths(cmd).is_some());
    }

    #[test]
    fn ignores_node_e_without_drive_path() {
        let cmd = r#"node -e "console.log('hello')""#;
        assert!(check_node_eval_backslash_paths(cmd).is_none());
    }

    #[test]
    fn ignores_drive_path_before_node_e() {
        let cmd = r#"cd C:\src && node -e "console.log('hello')""#;
        assert!(check_node_eval_backslash_paths(cmd).is_none());
    }

    // Pattern 4: Unquoted backslash paths
    #[test]
    fn detects_unquoted_ls() {
        let cmd = r"ls -la C:\src\codeflow";
        assert!(check_unquoted_backslash_paths(cmd).is_some());
    }

    #[test]
    fn detects_unquoted_rm() {
        let cmd = r"rm C:\src\codjiflo\main\file.json";
        assert!(check_unquoted_backslash_paths(cmd).is_some());
    }

    #[test]
    fn detects_unquoted_tail() {
        let cmd = r"tail -50 C:\Users\pedro\AppData\Local\Temp\output.txt";
        assert!(check_unquoted_backslash_paths(cmd).is_some());
    }

    #[test]
    fn allows_double_quoted_path() {
        let cmd = r#"ls -la "C:\src\project""#;
        assert!(check_unquoted_backslash_paths(cmd).is_none());
    }

    #[test]
    fn allows_single_quoted_path() {
        let cmd = r"ls -la 'C:\src\project'";
        assert!(check_unquoted_backslash_paths(cmd).is_none());
    }

    #[test]
    fn allows_forward_slash_path() {
        let cmd = "ls -la C:/src/project";
        assert!(check_unquoted_backslash_paths(cmd).is_none());
    }

    #[test]
    fn allows_doubled_backslash_path() {
        // C:\\ in raw command = C:\ after bash = valid
        let cmd = r"ls -la C:\\src\\project";
        assert!(check_unquoted_backslash_paths(cmd).is_none());
    }

    #[test]
    fn allows_unix_style_cd() {
        let cmd = "cd /c/src/project && ls";
        assert!(check_unquoted_backslash_paths(cmd).is_none());
    }
}
