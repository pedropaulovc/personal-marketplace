//! PreToolUse hook that auto-appends `| Out-Host` to PowerShell commands
//! whose output would otherwise be silently swallowed by the Claude Code
//! PowerShell tool's capture path. See anthropics/claude-code#59609.
//!
//! Strategy: on PreToolUse for tool `PowerShell`, inspect the command string.
//! If the heuristic `needs_out_host` decides the command's final result is
//! an unrendered object stream, rewrite the command to `<command> | Out-Host`
//! and re-submit via `hookSpecificOutput.updatedInput`. Otherwise no-op.
//!
//! Why `Out-Host` rather than `Out-String`: empirically the two produce
//! *different* bytes through the Claude Code PowerShell tool. `Out-Host` is
//! exactly the cmdlet pwsh.exe's implicit `Out-Default` routes to at end of
//! pipeline; the tool's capture path renders its output identically to what
//! a user gets from `| Format-Table` (byte-for-byte: same ANSI escapes, same
//! CRLF line endings, same column alignment). `Out-String` is a *parallel*
//! path that converts pipeline objects to strings without going through the
//! host renderer — it loses ANSI texture and produces a slightly different
//! alignment due to a leading-whitespace trim in the tool. Out-Host therefore
//! unifies the autofix output with the user's explicit `| Format-Table` cases.
//!
//! Failure mode: misclassifying as needing-a-fix only ever appends an extra
//! `| Out-Host` to already-rendered output, which is harmless and idempotent.

use serde_json::{json, Value};
use std::io::{self, Read};
use std::process;

/// Cmdlets / patterns that already produce final text output (or no output at
/// all). If the rightmost top-level pipeline segment starts with one of these,
/// the command does not need `| Out-String` appended.
const TERMINAL_CMDLETS: &[&str] = &[
    // Formatters
    "Format-Table", "Format-List", "Format-Wide", "Format-Custom",
    "ft", "fl", "fw", "fc",
    // Out-*
    "Out-String", "Out-Host", "Out-File", "Out-Null", "Out-GridView",
    "Out-Printer", "Out-Default",
    // Write-*
    "Write-Host", "Write-Output", "Write-Error", "Write-Warning",
    "Write-Information", "Write-Verbose", "Write-Debug",
    // Converters / serializers (already text)
    "ConvertTo-Json", "ConvertTo-Csv", "ConvertTo-Xml", "ConvertTo-Html",
    // Side-effecting consumers
    "Tee-Object",
];

fn main() {
    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        process::exit(0);
    }

    let payload: Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => process::exit(0),
    };

    if payload["tool_name"].as_str() != Some("PowerShell") {
        process::exit(0);
    }

    let tool_input = match payload["tool_input"].as_object() {
        Some(o) => o,
        None => process::exit(0),
    };

    let command = match tool_input.get("command").and_then(Value::as_str) {
        Some(c) if !c.trim().is_empty() => c,
        _ => process::exit(0),
    };

    if !needs_out_host(command) {
        process::exit(0);
    }

    let fixed = apply_fix(command);

    // Echo back the whole tool_input with the rewritten command.
    let mut updated = tool_input.clone();
    updated.insert("command".to_string(), Value::String(fixed));

    let out = json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "updatedInput": Value::Object(updated),
        }
    });

    println!("{}", out);
    process::exit(0);
}

/// Return the rightmost top-level pipeline segment of `command`, trimmed.
///
/// Naively splits on `|` while ignoring pipes inside single/double quotes.
/// Not a full PowerShell parser — sufficient for the command shapes Claude
/// typically emits in tool calls.
fn last_pipeline_segment(command: &str) -> &str {
    let bytes = command.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut last_pipe_end = 0usize;

    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b'|' if !in_single && !in_double => last_pipe_end = i + 1,
            _ => {}
        }
    }

    command[last_pipe_end..].trim()
}

/// Control-flow / declaration leading tokens that we conservatively skip
/// rather than try to reason about their output behaviour.
const CONTROL_LEADS: &[&str] = &[
    "if", "foreach", "for", "while", "do", "switch", "try",
    "function", "filter", "workflow", "class", "enum", "return",
];

/// Decide whether `command` would silently swallow object output and therefore
/// needs `| Out-Host` appended.
///
/// Strategy: bail conservatively on multi-statement / assignment / control-flow
/// shapes whose output semantics are tricky; otherwise inspect the rightmost
/// top-level pipeline segment and flag it unless it starts with a known
/// terminal cmdlet (case-insensitive).
fn needs_out_host(command: &str) -> bool {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return false;
    }

    if has_top_level_semicolon(trimmed) {
        return false;
    }

    let lead = trimmed
        .split(|c: char| c.is_whitespace() || c == '(' || c == '{')
        .next()
        .unwrap_or("");

    // Assignments: `$x = ...`, `$script:x = ...`, etc. Produce no stdout anyway.
    if lead.starts_with('$') && trimmed.contains('=') {
        return false;
    }

    if CONTROL_LEADS.iter().any(|k| k.eq_ignore_ascii_case(lead)) {
        return false;
    }

    let last = last_pipeline_segment(trimmed);
    let first_token = last
        .split(|c: char| c.is_whitespace() || c == '(' || c == '{' || c == ';')
        .next()
        .unwrap_or("");

    !TERMINAL_CMDLETS
        .iter()
        .any(|c| c.eq_ignore_ascii_case(first_token))
}

/// True iff `s` contains a `;` outside any single/double quote, parenthesis,
/// brace, or bracket. The semicolon must be a *real* top-level statement
/// separator — semicolons inside hashtable literals (`@{a=1; b=2}`), script
/// blocks (`{ ... ; ... }`), subexpressions (`$(... ; ...)`), or array
/// literals (`@(...; ...)`) must not trip the bailout.
fn has_top_level_semicolon(s: &str) -> bool {
    let mut in_single = false;
    let mut in_double = false;
    let mut paren = 0i32;
    let mut brace = 0i32;
    let mut bracket = 0i32;
    for &b in s.as_bytes() {
        // Quote state takes precedence — characters inside quotes don't change
        // any other state.
        match b {
            b'\'' if !in_double => {
                in_single = !in_single;
                continue;
            }
            b'"' if !in_single => {
                in_double = !in_double;
                continue;
            }
            _ if in_single || in_double => continue,
            _ => {}
        }
        match b {
            b'(' => paren += 1,
            b')' if paren > 0 => paren -= 1,
            b'{' => brace += 1,
            b'}' if brace > 0 => brace -= 1,
            b'[' => bracket += 1,
            b']' if bracket > 0 => bracket -= 1,
            b';' if paren == 0 && brace == 0 && bracket == 0 => return true,
            _ => {}
        }
    }
    false
}

/// Append `| Out-Host` to the command, preserving any trailing whitespace.
fn apply_fix(command: &str) -> String {
    let trimmed = command.trim_end();
    let suffix = &command[trimmed.len()..];
    format!("{} | Out-Host{}", trimmed, suffix)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_segment_simple() {
        assert_eq!(last_pipeline_segment("Get-Process"), "Get-Process");
    }

    #[test]
    fn last_segment_pipeline() {
        assert_eq!(
            last_pipeline_segment("Get-Process | Select-Object Id, Name"),
            "Select-Object Id, Name"
        );
    }

    #[test]
    fn last_segment_ignores_quoted_pipe() {
        assert_eq!(
            last_pipeline_segment("Write-Output 'a | b' | Select-Object"),
            "Select-Object"
        );
        assert_eq!(
            last_pipeline_segment("Write-Output 'a | b'"),
            "Write-Output 'a | b'"
        );
    }

    #[test]
    fn apply_fix_appends_out_host() {
        assert_eq!(
            apply_fix("Get-Process | Select-Object Id"),
            "Get-Process | Select-Object Id | Out-Host"
        );
    }

    #[test]
    fn apply_fix_preserves_trailing_newline() {
        assert_eq!(
            apply_fix("Get-Process\n"),
            "Get-Process | Out-Host\n"
        );
    }

    // -- Heuristic ------------------------------------------------------------

    #[test]
    fn flags_bare_get_process() {
        assert!(needs_out_host("Get-Process"));
    }

    #[test]
    fn flags_get_process_select() {
        assert!(needs_out_host(
            "Get-Process -Name explorer -ErrorAction SilentlyContinue | Select-Object Id, ProcessName, StartTime"
        ));
    }

    #[test]
    fn skips_when_format_table_terminal() {
        assert!(!needs_out_host(
            "Get-Process | Select-Object Id, ProcessName | Format-Table"
        ));
    }

    #[test]
    fn skips_when_out_string_already_present() {
        // Out-String stays in TERMINAL_CMDLETS — if the user explicitly chose
        // Out-String, don't override it.
        assert!(!needs_out_host("Get-Process | Out-String"));
    }

    #[test]
    fn skips_when_out_host_already_present() {
        // Idempotence: Out-Host is what apply_fix appends. A second pass must
        // not double-wrap (Out-Host consumes its input — re-wrapping makes the
        // tool result empty).
        assert!(!needs_out_host("Get-Process | Out-Host"));
        assert!(!needs_out_host("Get-Process | Select-Object Id | Out-Host"));
    }

    #[test]
    fn case_insensitive_terminal_cmdlet() {
        assert!(!needs_out_host("Get-Process | format-table"));
        assert!(!needs_out_host("Get-Process | FORMAT-TABLE"));
    }

    #[test]
    fn skips_format_table_alias_ft() {
        assert!(!needs_out_host("Get-Process | ft"));
    }

    #[test]
    fn skips_out_null() {
        assert!(!needs_out_host("Get-Process | Out-Null"));
    }

    #[test]
    fn skips_convertto_json() {
        assert!(!needs_out_host("Get-Process | ConvertTo-Json"));
    }

    #[test]
    fn skips_assignment() {
        assert!(!needs_out_host("$procs = Get-Process"));
        assert!(!needs_out_host("$global:x = Get-Process | Select-Object Id"));
    }

    #[test]
    fn skips_control_flow() {
        assert!(!needs_out_host("foreach ($p in Get-Process) { $p.Name }"));
        assert!(!needs_out_host("if ($true) { Get-Process }"));
        assert!(!needs_out_host("while ($x -lt 5) { $x++ }"));
    }

    #[test]
    fn skips_semicolon_separated_statements() {
        assert!(!needs_out_host("Get-Process; Get-Service"));
    }

    #[test]
    fn semicolon_inside_quotes_does_not_bail() {
        assert!(needs_out_host("Write-Output 'a;b' | Select-Object"));
    }

    #[test]
    fn flags_where_object_terminal() {
        assert!(needs_out_host(
            "Get-Process | Where-Object { $_.CPU -gt 10 }"
        ));
    }

    #[test]
    fn flags_get_childitem() {
        assert!(needs_out_host("Get-ChildItem . -Recurse"));
    }

    #[test]
    fn empty_or_whitespace_returns_false() {
        assert!(!needs_out_host(""));
        assert!(!needs_out_host("   "));
        assert!(!needs_out_host("\n\t"));
    }

    #[test]
    fn quoted_pipe_does_not_split_last_segment() {
        // `'a | b'` is one token; the real last segment is `Select-Object`.
        assert!(needs_out_host("Write-Output 'a | b' | Select-Object"));
    }

    #[test]
    fn format_tableish_does_not_match_format_table() {
        // Sanity: `Format-Tableish` (hypothetical) must not be treated as terminal.
        assert!(needs_out_host("Get-Process | Format-Tableish"));
    }

    // -- Brace/paren/bracket-aware semicolon handling -------------------------

    #[test]
    fn semicolon_inside_braces_does_not_bail() {
        // Hashtable literals contain `;` between entries. Without the
        // depth-tracking fix, this case was a false-positive bailout and the
        // hook produced no output, reproducing the original bug.
        assert!(needs_out_host(
            "Get-Process | Select-Object @{N='ProcID'; E={$_.Id}}, ProcessName"
        ));
    }

    #[test]
    fn semicolon_inside_parens_does_not_bail() {
        // Subexpression with multiple statements.
        assert!(needs_out_host("Get-Process | Where-Object { $_.Id -gt 100 }"));
        assert!(needs_out_host(
            "$(Get-Process; Get-Service | Select-Object -First 1)"
        ));
    }

    #[test]
    fn semicolon_inside_array_literal_does_not_bail() {
        assert!(needs_out_host("@(1; 2; 3) | Measure-Object -Sum"));
    }

    #[test]
    fn top_level_semicolon_still_bails() {
        // Sanity: the brace/paren tracking must not break the original bailout.
        assert!(!needs_out_host("Get-Process; Get-Service"));
        assert!(!needs_out_host(
            "function Get-PyVersion { python --version }; Get-PyVersion"
        ));
    }

    #[test]
    fn unmatched_braces_do_not_underflow() {
        // Defensive: a stray `}` shouldn't make depth negative and re-enable
        // bailout. Use a top-level `;` after the stray brace.
        assert!(!needs_out_host("}; Get-Process"));
    }

    // -- External commands (the python script use case) ----------------------

    #[test]
    fn flags_external_command_python_script() {
        assert!(needs_out_host("python script.py"));
        assert!(needs_out_host(r"python C:\tmp\script.py"));
    }

    #[test]
    fn flags_external_command_python_inline() {
        assert!(needs_out_host(r#"python -c "print('hello')""#));
    }

    #[test]
    fn flags_external_command_with_args() {
        assert!(needs_out_host("python script.py --arg1 value1 --arg2 value2"));
    }

    #[test]
    fn flags_external_call_operator() {
        assert!(needs_out_host(r#"& "C:\Program Files\app.exe" arg"#));
        assert!(needs_out_host("& { Get-Process }"));
    }

    // -- Pipelines ending in object-producing cmdlets ------------------------

    #[test]
    fn flags_select_string_pipeline() {
        assert!(needs_out_host("python script.py | Select-String 'pattern'"));
    }

    #[test]
    fn flags_convert_from_json_pipeline() {
        // Critical: without the hook, ConvertFrom-Json output is silent. The
        // hook makes it visible — same case as Get-Process raw.
        assert!(needs_out_host("python -c \"print('{}')\" | ConvertFrom-Json"));
    }

    #[test]
    fn flags_foreach_object_pipeline() {
        assert!(needs_out_host(
            "1..5 | ForEach-Object { python -c \"print($_)\" }"
        ));
    }

    #[test]
    fn flags_measure_object() {
        assert!(needs_out_host("@(1,2,3) | Measure-Object -Sum"));
    }

    #[test]
    fn flags_sort_object_pipeline() {
        assert!(needs_out_host("Get-Process | Sort-Object CPU -Descending"));
    }

    #[test]
    fn flags_group_object_pipeline() {
        assert!(needs_out_host("Get-Process | Group-Object Company"));
    }

    // -- Expressions and method/property access ------------------------------

    #[test]
    fn flags_parenthesized_count() {
        assert!(needs_out_host("(Get-Process | Select-Object Id, Name).Count"));
    }

    #[test]
    fn flags_dotnet_static_property() {
        assert!(needs_out_host("[Math]::PI"));
        assert!(needs_out_host("[Environment]::OSVersion"));
    }

    #[test]
    fn flags_dotnet_method_call() {
        assert!(needs_out_host("[Math]::Pow(2, 10)"));
    }

    #[test]
    fn flags_range_operator() {
        assert!(needs_out_host("1..5"));
    }

    #[test]
    fn flags_env_var_access() {
        assert!(needs_out_host("$env:USERNAME"));
        assert!(needs_out_host("$LASTEXITCODE"));
    }

    // -- Control flow bailouts -----------------------------------------------

    #[test]
    fn skips_try_catch_block() {
        assert!(!needs_out_host(
            "try { python script.py 2>&1 | Out-String } catch { \"caught: $_\" }"
        ));
    }

    #[test]
    fn skips_function_definition_then_call() {
        // The top-level `;` between the function decl and the call already
        // forces a bailout — but make sure both halves don't accidentally
        // re-enable the heuristic.
        assert!(!needs_out_host(
            "function Get-PyVersion { python --version }; Get-PyVersion"
        ));
    }

    // -- Already-handled terminator cases ------------------------------------

    #[test]
    fn skips_format_table_after_long_pipeline() {
        assert!(!needs_out_host(
            "Get-Process | Sort-Object CPU -Descending | Select-Object -First 5 | Format-Table"
        ));
    }
}
