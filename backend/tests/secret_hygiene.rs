//! Guards for the `secret-hygiene` change (RED phase).
//!
//! Spec: `openspec/changes/secret-hygiene/specs/secrets/spec.md`
//! → Requirement «El secreto de firma nunca se escribe en el log».
//!
//! Both properties are enforced over *source text*, so they need neither a
//! running server nor a database:
//!
//! 1. No log invocation in `backend/src` interpolates an identifier carrying the
//!    word `secret`. The contract forbids the value, a truncation of it **and**
//!    its length (`debug!("{}", secret.len())` must trip the guard).
//! 2. The default `RUST_LOG` shipped in `compose.yml` does not enable debug, so
//!    a stray `debug!` cannot publish a credential in a standard deployment.
//!
//! Every path is derived from `CARGO_MANIFEST_DIR`, so the guards do not depend
//! on the directory `cargo test` is launched from.
use regex::Regex;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

/// Macros whose argument ends up in the log or on a stream.
///
/// `dbg!` dumps its argument to stderr *and* echoes it back into the source,
/// and `clippy::dbg_macro` lives in the `restriction` group — which this crate
/// does not enable —, so nothing else would stop it. `print!` and `eprint!`
/// are the write-half of `println!`/`eprintln!`.
const LOG_MACROS: [&str; 10] = [
    "debug", "info", "warn", "error", "trace", "println", "eprintln", "print", "eprint", "dbg",
];

/// Any identifier that carries the word `secret`, in any case.
const SECRET_IDENTIFIER: &str = r"(?i)[A-Za-z_]*secret[A-Za-z_]*";

/// Collects, recursively, every `.rs` file below `dir`.
fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension() == Some(OsStr::new("rs")) {
            out.push(path);
        }
    }
}

/// Collapses every run of whitespace into a single space, so a log invocation
/// spread over several lines is scanned as a single unit.
fn normalize(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Returns the tail of a raw string starting right after its opening quote.
fn skip_raw_string_tail(bytes: &[u8], mut index: usize, hashes: usize) -> usize {
    while index < bytes.len() {
        if bytes[index] == b'"' {
            let mut seen = 0;
            while seen < hashes && bytes.get(index + 1 + seen) == Some(&b'#') {
                seen += 1;
            }
            if seen == hashes {
                return index + 1 + hashes;
            }
        }
        index += 1;
    }
    index
}

/// Removes string literals —plain and raw— from `source`.
///
/// The word `secret` inside a *message* is not an interpolation of the value:
/// the weak-secret warning mandated by the spec literally reads «SECRET is
/// shorter than the 32 characters…», and it must not trip the guard. Stripping
/// the literals leaves the check on identifiers only.
fn strip_string_literals(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut stripped: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                index += 1;
                while index < bytes.len() {
                    match bytes[index] {
                        // An escaped quote does not close the literal.
                        b'\\' => index += 2,
                        b'"' => {
                            index += 1;
                            break;
                        },
                        _ => index += 1,
                    }
                }
            },
            b'r' if matches!(bytes.get(index + 1), Some(b'"' | b'#')) => {
                let mut cursor = index + 1;
                let mut hashes = 0;
                while bytes.get(cursor) == Some(&b'#') {
                    hashes += 1;
                    cursor += 1;
                }
                if bytes.get(cursor) == Some(&b'"') {
                    index = skip_raw_string_tail(bytes, cursor + 1, hashes);
                } else {
                    // A raw identifier (`r#type`), not a raw string.
                    stripped.push(b'r');
                    index += 1;
                }
            },
            byte => {
                stripped.push(byte);
                index += 1;
            },
        }
    }
    String::from_utf8(stripped).expect("the unstripped bytes stay valid UTF-8")
}

/// Returns the index of the parenthesis closing the one at `open`.
fn matching_parenthesis(source: &str, open: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut index = open;
    let mut in_string = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if byte == b'\\' {
                index += 2;
                continue;
            }
            if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(index);
                    }
                },
                _ => {},
            }
        }
        index += 1;
    }
    None
}

/// Returns `(macro name, argument)` for every log invocation in `source`.
///
/// The argument is delimited by counting parentheses, so an argument that itself
/// calls something — `info!("{}", ip_of(request))` — is not truncated.
fn log_macro_arguments(source: &str) -> Vec<(String, String)> {
    let mut calls = Vec::new();
    for name in LOG_MACROS {
        let needle = format!("{name}!");
        let mut from = 0;
        while let Some(offset) = source[from..].find(&needle) {
            let name_start = from + offset;
            let after_bang = name_start + needle.len();
            // `println!` is a suffix of `eprintln!`: reject a match that is the
            // tail of a longer identifier.
            let is_own_name = source[..name_start]
                .chars()
                .next_back()
                .is_none_or(|previous| !previous.is_alphanumeric() && previous != '_');
            if is_own_name {
                let rest = &source[after_bang..];
                let open = after_bang + (rest.len() - rest.trim_start().len());
                if source.as_bytes().get(open) == Some(&b'(')
                    && let Some(close) = matching_parenthesis(source, open)
                {
                    calls.push((name.to_string(), source[open + 1..close].to_string()));
                }
            }
            from = after_bang;
        }
    }
    calls
}

/// Returns every log invocation whose argument interpolates an identifier
/// matching [`SECRET_IDENTIFIER`], as `(macro name, argument)` pairs.
fn secret_interpolations(source: &str) -> Vec<(String, String)> {
    let secret = Regex::new(SECRET_IDENTIFIER).expect("the pattern is a valid regex");
    log_macro_arguments(&normalize(source))
        .into_iter()
        .filter(|(_, argument)| secret.is_match(&strip_string_literals(argument)))
        .collect()
}

/// The signing secret must never reach the log: not its value, not truncated,
/// not even its length. Today this fails on `src/main.rs:88`.
#[test]
fn ninguna_invocacion_de_log_interpola_el_secreto() {
    let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    rust_sources(&source_dir, &mut sources);
    assert!(
        !sources.is_empty(),
        "no `.rs` source found below {}",
        source_dir.display()
    );

    let mut violations = Vec::new();
    for path in &sources {
        let source = fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        for (name, argument) in secret_interpolations(&source) {
            violations.push(format!("{}: {name}!({argument})", path.display()));
        }
    }

    assert!(
        violations.is_empty(),
        "a log invocation interpolates the signing secret (value, truncation and \
         length are all forbidden):\n{}",
        violations.join("\n")
    );
}

/// Positive control: an interpolation of `secret` *is* detected, so the guard
/// above cannot pass because it fails to detect anything at all.
#[test]
fn un_argumento_que_interpola_el_secreto_se_detecta() {
    let source = r#"debug!("Secret: {}", secret);"#;
    let found = secret_interpolations(source);
    assert_eq!(
        found.len(),
        1,
        "the interpolation of `secret` must be detected, got {found:?}"
    );
    assert_eq!(found[0].0, "debug");
}

/// Positive control: the direct dumps that skip `tracing` altogether are covered
/// too. `dbg!` writes the value to stderr and echoes it back into the source,
/// and it is the macro most likely to reach a debugger's hand, so a guard that
/// ignores it leaves the cheapest regression path open.
#[test]
fn un_volcado_con_dbg_se_detecta() {
    let source = r"dbg!(secret);";
    let found = secret_interpolations(source);
    assert_eq!(
        found.len(),
        1,
        "`dbg!` over `secret` must be detected, got {found:?}"
    );
    assert_eq!(found[0].0, "dbg");
}

/// Positive control: `print!` writes to stdout without going through the logger,
/// so it must be scanned like `println!`.
#[test]
fn una_impresion_directa_se_detecta() {
    let source = r#"print!("{}", secret);"#;
    let found = secret_interpolations(source);
    assert_eq!(
        found.len(),
        1,
        "`print!` over `secret` must be detected, got {found:?}"
    );
    assert_eq!(found[0].0, "print");
}

/// Positive control: `eprint!` writes to stderr —where `dbg!` goes too— and is
/// the write-half of `eprintln!`.
#[test]
fn una_impresion_directa_a_stderr_se_detecta() {
    let source = r#"eprint!("{}", secret);"#;
    let found = secret_interpolations(source);
    assert_eq!(
        found.len(),
        1,
        "`eprint!` over `secret` must be detected, got {found:?}"
    );
    assert_eq!(found[0].0, "eprint");
}

/// Adding `print` and `eprint` must not make `eprintln!` count twice: `print!`
/// never matches inside `eprintln!` (the character after `print` is `l`, not
/// `!`), `eprint!` never matches inside `eprintln!` either (same reason), and
/// the `print!` that *does* sit inside `eprint!` is rejected as the tail of a
/// longer identifier. One invocation, one hit.
#[test]
fn una_invocacion_de_eprintln_se_cuenta_una_sola_vez() {
    let source = r#"eprintln!("{}", secret);"#;
    let found = secret_interpolations(source);
    assert_eq!(
        found.len(),
        1,
        "`eprintln!` must be counted exactly once, got {found:?}"
    );
    assert_eq!(found[0].0, "eprintln");
}

/// Mirror of the control above: `println!` stays at one hit after `print` joins
/// the list — `print!` is not a substring of `println!`.
#[test]
fn una_invocacion_de_println_se_cuenta_una_sola_vez() {
    let source = r#"println!("{}", secret);"#;
    let found = secret_interpolations(source);
    assert_eq!(
        found.len(),
        1,
        "`println!` must be counted exactly once, got {found:?}"
    );
    assert_eq!(found[0].0, "println");
}

/// Negative control: the warning required by the spec *names* the requirement,
/// so its literal contains the word `SECRET`. A guard that flagged it would
/// force the fix to drop the message; stripping string literals keeps the
/// predicate on interpolations only, which is exactly the contract.
#[test]
fn un_aviso_que_solo_menciona_la_palabra_secreto_no_es_violacion() {
    let source = r#"warn!("SECRET is shorter than the 32 characters required");"#;
    assert!(
        secret_interpolations(source).is_empty(),
        "the message of the weak-secret warning must not be flagged"
    );
}

/// The default configuration of the image must not enable debug logging. The
/// literal value is not asserted: adding another target to the filter is fine,
/// what matters is that debug stays off. Today `compose.yml` reads
/// `RUST_LOG: backend=debug,maxminddb=warn`, so this fails.
#[test]
fn la_depuracion_no_esta_activa_por_defecto_en_el_compose() {
    let compose = Path::new(env!("CARGO_MANIFEST_DIR")).join("../compose.yml");
    let source = fs::read_to_string(&compose)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", compose.display()));
    let line = source
        .lines()
        .find(|line| line.trim_start().starts_with("RUST_LOG:"))
        .expect("compose.yml declares a default RUST_LOG");

    assert!(
        !line.to_ascii_lowercase().contains("debug"),
        "the default RUST_LOG enables debug: {line}"
    );
}

/// The other half of the scenario: the `backend` target must run at `info`.
/// The whole filter is not pinned — other targets are welcome — and spaces
/// around the `=` and the commas are tolerated, so adding an objective does not
/// break the guard. Today this passes because `compose.yml` already reads
/// `backend=info,maxminddb=warn`; the assertion is there so it cannot silently
/// drift to `warn`.
#[test]
fn el_objetivo_backend_corre_en_info_por_defecto_en_el_compose() {
    let compose = Path::new(env!("CARGO_MANIFEST_DIR")).join("../compose.yml");
    let source = fs::read_to_string(&compose)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", compose.display()));
    let line = source
        .lines()
        .find(|line| line.trim_start().starts_with("RUST_LOG:"))
        .expect("compose.yml declares a default RUST_LOG");
    let filter = line
        .split_once(':')
        .map(|(_, rest)| rest)
        .expect("the RUST_LOG declaration has a `:` separator");

    let level = filter
        .split(',')
        .filter_map(|directive| directive.split_once('='))
        .find(|(target, _)| target.trim() == "backend")
        .map(|(_, level)| level.trim())
        .expect("the default RUST_LOG must set a level for the `backend` target");

    assert_eq!(
        level, "info",
        "the default RUST_LOG must run the `backend` target at `info`: {line}"
    );
}
