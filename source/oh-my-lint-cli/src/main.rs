//! Command-line interface for checking and fixing SQL blocks and documenting rules.

const SKIP_DIRECTORIES: &[&str] = &[".git", ".venv", "__pycache__", ".ruff_cache", "build", "dist", "node_modules", "target"];

/// Main command.
#[derive(clap::Parser)]
#[command(name = "oh-my-lint", version, about = "Check code against house rules and fix only the affected syntax blocks")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Check SQL blocks in files or directories.
    Check(CheckArgs),
    /// Fix SQL blocks. Defaults to a dry run.
    Fix(FixArgs),
    /// List built-in rule families and their rule IDs.
    Rules(RulesArgs),
    /// Show documentation for a rule family or rule ID.
    Explain(ExplainArgs),
}

/// SQL dialect to parse.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum DialectArg {
    Postgres,
    Mysql,
    #[value(name = "duckdb")]
    DuckDb,
    Hive,
}

impl From<DialectArg> for oh_my_lint_lib::sql::Dialect {
    fn from(value: DialectArg) -> Self {
        match value {
            DialectArg::Postgres => Self::Postgres,
            DialectArg::Mysql => Self::Mysql,
            DialectArg::DuckDb => Self::DuckDb,
            DialectArg::Hive => Self::Hive,
        }
    }
}

#[derive(clap::Args)]
struct CheckArgs {
    /// Files or directories to check. Defaults to the current directory.
    #[arg(default_value = ".")]
    paths: Vec<std::path::PathBuf>,
    /// SQL dialect to parse.
    #[arg(long, value_enum, default_value = "postgres")]
    dialect: DialectArg,
    /// Output JSON.
    #[arg(long)]
    json: bool,
    /// Show counts by rule.
    #[arg(long)]
    statistics: bool,
    /// Maximum number of warnings allowed before the check fails.
    #[arg(long, default_value_t = 0)]
    max_warnings: usize,
}

#[derive(clap::Args)]
struct FixArgs {
    /// Files or directories to fix. Defaults to the current directory.
    #[arg(default_value = ".")]
    paths: Vec<std::path::PathBuf>,
    /// SQL dialect to parse.
    #[arg(long, value_enum, default_value = "postgres")]
    dialect: DialectArg,
    /// Fix a single file, optionally limited by --range.
    #[arg(long, conflicts_with = "stdin")]
    file: Option<std::path::PathBuf>,
    /// Fix only blocks within the L:C-L:C range.
    #[arg(long)]
    range: Option<String>,
    /// Read code from standard input.
    #[arg(long, conflicts_with_all = ["file", "write"])]
    stdin: bool,
    /// Filename used to detect the language of standard input.
    #[arg(long)]
    stdin_filename: Option<std::path::PathBuf>,
    /// Set the input language explicitly. Only sql is supported.
    #[arg(long)]
    lang: Option<String>,
    /// Preview fixes without writing files.
    #[arg(long, conflicts_with = "write")]
    #[allow(dead_code)]
    dry_run: bool,
    /// Output a unified diff.
    #[arg(long)]
    diff: bool,
    /// Write fixes back to files.
    #[arg(long)]
    write: bool,
    /// Output JSON.
    #[arg(long)]
    json: bool,
    /// Allow unsafe fixes.
    #[arg(long = "unsafe")]
    unsafe_fixes: bool,
}

#[derive(clap::Args)]
struct RulesArgs {
    /// Output JSON.
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct ExplainArgs {
    /// Rule family or rule ID to look up.
    rule: String,
}

#[derive(serde::Serialize)]
struct FindingOut {
    file: String,
    line: usize,
    column: usize,
    rule: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    severity: String,
    message: String,
    fixable: bool,
}

#[derive(serde::Serialize)]
struct SummaryOut {
    files: usize,
    blocks: usize,
    findings: usize,
}

#[derive(serde::Serialize)]
struct CheckOut {
    findings: Vec<FindingOut>,
    summary: SummaryOut,
}

#[derive(serde::Serialize)]
struct SkippedOut {
    rule: String,
    reason: String,
}

#[derive(serde::Serialize)]
struct FixOut {
    file: String,
    block_range: String,
    fixed_text: String,
    patch: String,
    applied: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    applied_codes: Vec<String>,
    skipped: Vec<SkippedOut>,
    remaining: Vec<FindingOut>,
    written: bool,
}

impl FixOut {
    fn print_applied_codes(&self) {
        if !self.applied_codes.is_empty() {
            eprintln!("{} [{}] applied_codes={:?}", self.file, self.block_range, self.applied_codes);
        }
    }
}

#[derive(serde::Serialize)]
struct RuleOut {
    id: &'static str,
    description: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    codes: Option<&'static [oh_my_lint_lib::model::RuleDoc]>,
}

fn main() -> std::process::ExitCode {
    let cli = <Cli as clap::Parser>::parse();
    match run(cli) {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            eprintln!("oh-my-lint: {error}");
            std::process::ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> oh_my_lint_lib::error::Result<u8> {
    match cli.command {
        Command::Check(args) => {
            let engine = engine_for(args.dialect.into());
            check(&engine, args)
        }
        Command::Fix(args) => {
            let engine = engine_for(args.dialect.into());
            fix(&engine, args)
        }
        Command::Rules(args) => rules(&oh_my_lint_lib::engine::Engine::with_defaults(), args),
        Command::Explain(args) => explain(&oh_my_lint_lib::engine::Engine::with_defaults(), args),
    }
}

fn engine_for(dialect: oh_my_lint_lib::sql::Dialect) -> oh_my_lint_lib::engine::Engine {
    oh_my_lint_lib::engine::Engine::new(vec![Box::new(oh_my_lint_lib::sql::SqlRule::new(dialect))])
}

fn check(engine: &oh_my_lint_lib::engine::Engine, args: CheckArgs) -> oh_my_lint_lib::error::Result<u8> {
    let documents = load_paths(&args.paths)?;
    let mut findings = Vec::new();
    let mut blocks = 0usize;
    for document in &documents {
        blocks += document.blocks.len();
        for located in engine.check(document) {
            let offset = located.block_span.start + located.finding.span.start;
            let (line, column) = document.line_col(offset);
            findings.push(FindingOut {
                file: display_path(&document.path),
                line,
                column,
                rule: located.finding.rule,
                code: located.finding.code,
                severity: severity_name(located.finding.severity).to_owned(),
                message: located.finding.message,
                fixable: located.fixable,
            });
        }
    }

    if args.statistics {
        let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        for finding in &findings {
            *counts.entry(finding.code.as_deref().unwrap_or(&finding.rule)).or_default() += 1;
        }
        for (rule, count) in counts {
            if args.json {
                eprintln!("{count:>6}  {rule}");
            } else {
                println!("{count:>6}  {rule}");
            }
        }
    }

    let total = findings.len();
    if args.json {
        let output = CheckOut { findings, summary: SummaryOut { files: documents.len(), blocks, findings: total } };
        println!("{}", serde_json::to_string_pretty(&output).map_err(json_error)?);
    } else {
        for finding in &findings {
            let (separator, code) = finding.code.as_deref().map_or(("", ""), |code| (" ", code));
            println!(
                "{}:{}:{}: {}{separator}{code} {}: {}",
                finding.file, finding.line, finding.column, finding.rule, finding.severity, finding.message
            );
        }
        println!("checked {} files, {} blocks, {} findings", documents.len(), blocks, total);
    }

    if total > args.max_warnings { Ok(1) } else { Ok(0) }
}

fn fix(engine: &oh_my_lint_lib::engine::Engine, args: FixArgs) -> oh_my_lint_lib::error::Result<u8> {
    if args.stdin {
        return fix_stdin(engine, &args);
    }
    if let Some(file) = &args.file {
        return fix_file(engine, file, args.range.as_deref(), &args);
    }
    if args.range.is_some() {
        return Err(oh_my_lint_lib::error::Error::Rejected("--range requires --file or --stdin".to_owned()));
    }
    fix_paths(engine, &args)
}

fn fix_stdin(engine: &oh_my_lint_lib::engine::Engine, args: &FixArgs) -> oh_my_lint_lib::error::Result<u8> {
    let mut text = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)?;
    let path = stdin_path(args)?;
    let document = oh_my_lint_lib::source::Document::from_text(path, text)?;
    let range = args.range.as_deref().map(|spec| document.parse_range(spec)).transpose()?;
    let fixes = engine.apply(&document, range, args.unsafe_fixes);
    emit_fixes(&document, fixes, args, false, true)
}

fn fix_file(engine: &oh_my_lint_lib::engine::Engine, file: &std::path::Path, range: Option<&str>, args: &FixArgs) -> oh_my_lint_lib::error::Result<u8> {
    let document = oh_my_lint_lib::source::Document::load(file)?;
    let snippet = range.is_some();
    let range = range.map(|spec| document.parse_range(spec)).transpose()?;
    let fixes = engine.apply(&document, range, args.unsafe_fixes);
    emit_fixes(&document, fixes, args, args.write, snippet)
}

fn fix_paths(engine: &oh_my_lint_lib::engine::Engine, args: &FixArgs) -> oh_my_lint_lib::error::Result<u8> {
    let documents = load_paths(&args.paths)?;
    let mut outcomes = Vec::new();
    for document in &documents {
        let fixes = engine.apply(document, None, args.unsafe_fixes);
        if fixes.is_empty() {
            if args.write {
                let text = oh_my_lint_lib::engine::normalize_file_end(&document.text);
                if text != document.text {
                    write_atomic(&document.path, &text)?;
                }
            }
            continue;
        }
        if args.write {
            let text = oh_my_lint_lib::engine::splice(&document.text, &fixes);
            write_atomic(&document.path, &text)?;
        }
        outcomes.extend(outputs(document, fixes, args.write));
    }
    print_outcomes(&outcomes, args);
    let changed = outcomes.iter().any(|outcome| !outcome.patch.is_empty());
    let remaining: usize = outcomes.iter().map(|outcome| outcome.remaining.len()).sum();
    if remaining > 0 {
        return Ok(1);
    }
    if args.write { Ok(0) } else { Ok(if changed { 1 } else { 0 }) }
}

fn emit_fixes(document: &oh_my_lint_lib::source::Document, fixes: Vec<oh_my_lint_lib::engine::FileFix>, args: &FixArgs, write: bool, snippet: bool) -> oh_my_lint_lib::error::Result<u8> {
    if write && !fixes.is_empty() {
        let text = oh_my_lint_lib::engine::splice(&document.text, &fixes);
        write_atomic(&document.path, &text)?;
    }
    let outputs = outputs(document, fixes, write);
    if args.json {
        println!("{}", serde_json::to_string_pretty(&outputs).map_err(json_error)?);
    } else if snippet && !args.diff {
        for outcome in &outputs {
            println!("{}", outcome.fixed_text);
            outcome.print_applied_codes();
        }
    } else {
        print_outcomes(&outputs, args);
    }
    let changed = outputs.iter().any(|outcome| !outcome.patch.is_empty());
    let remaining: usize = outputs.iter().map(|outcome| outcome.remaining.len()).sum();
    let failed = remaining > 0 || (changed && !write);
    Ok(if failed { 1 } else { 0 })
}

fn outputs(document: &oh_my_lint_lib::source::Document, fixes: Vec<oh_my_lint_lib::engine::FileFix>, written: bool) -> Vec<FixOut> {
    fixes
        .into_iter()
        .map(|fix| {
            let fixed_document = oh_my_lint_lib::source::Document { path: fix.path, text: fix.fixed, blocks: Vec::new() };
            let (start_line, start_column) = document.line_col(fix.block_span.start);
            let (end_line, end_column) = document.line_col(fix.block_span.end);
            let remaining = fix.remaining.into_iter().map(|finding| finding_output(Some(&fixed_document), finding)).collect();
            FixOut {
                file: display_path(&document.path),
                block_range: format!("{start_line}:{start_column}-{end_line}:{end_column}"),
                patch: oh_my_lint_lib::diff::unified(&display_path(&document.path), &fix.original, &fixed_document.text),
                fixed_text: fixed_document.text,
                applied: fix.applied,
                applied_codes: fix.applied_codes,
                skipped: fix.skipped.into_iter().map(|skip| SkippedOut { rule: skip.rule, reason: skip.reason }).collect(),
                remaining,
                written,
            }
        })
        .collect()
}

fn finding_output(document: Option<&oh_my_lint_lib::source::Document>, finding: oh_my_lint_lib::model::Finding) -> FindingOut {
    let (line, column) = document.map_or((1, 1), |document| document.line_col(finding.span.start));
    FindingOut {
        file: document.map_or_else(|| "<fixed>".to_owned(), |document| display_path(&document.path)),
        line,
        column,
        rule: finding.rule,
        code: finding.code,
        severity: severity_name(finding.severity).to_owned(),
        message: finding.message,
        fixable: finding.fix.is_some(),
    }
}

fn print_outcomes(outcomes: &[FixOut], args: &FixArgs) {
    if args.json {
        if let Ok(text) = serde_json::to_string_pretty(outcomes) {
            println!("{text}");
        }
        return;
    }
    if args.diff {
        for outcome in outcomes {
            print!("{}", outcome.patch);
            outcome.print_applied_codes();
        }
        return;
    }
    for outcome in outcomes {
        let action = if outcome.applied.is_empty() {
            "skipped"
        } else if outcome.written {
            "wrote"
        } else {
            "would change"
        };
        print!(
            "{action} {} [{}] applied={:?} skipped={:?}",
            outcome.file,
            outcome.block_range,
            outcome.applied,
            outcome.skipped.iter().map(|skip| skip.rule.as_str()).collect::<Vec<_>>()
        );
        if !outcome.applied_codes.is_empty() {
            print!(" applied_codes={:?}", outcome.applied_codes);
        }
        println!();
    }
    if outcomes.is_empty() {
        println!("no changes");
    }
}

fn rules(engine: &oh_my_lint_lib::engine::Engine, args: RulesArgs) -> oh_my_lint_lib::error::Result<u8> {
    let rules: Vec<RuleOut> = engine
        .rules()
        .map(|rule| {
            let codes = rule.codes();
            RuleOut { id: rule.id(), description: rule.description(), codes: (!codes.is_empty()).then_some(codes) }
        })
        .collect();
    if args.json {
        println!("{}", serde_json::to_string_pretty(&rules).map_err(json_error)?);
    } else {
        for rule in rules {
            println!("{:<16} {}", rule.id, rule.description);
            for doc in rule.codes.into_iter().flatten() {
                println!("  {:<14} {}", doc.code, doc.description);
            }
        }
    }
    Ok(0)
}

fn explain(engine: &oh_my_lint_lib::engine::Engine, args: ExplainArgs) -> oh_my_lint_lib::error::Result<u8> {
    if let Some(rule) = engine.rules().find(|rule| rule.id() == args.rule) {
        println!("{}", rule.id());
        println!("{}", rule.description());
        return Ok(0);
    }
    let Some(doc) = engine.rules().flat_map(|rule| rule.codes()).find(|doc| doc.code == args.rule) else {
        return Err(oh_my_lint_lib::error::Error::Rejected(format!("unknown rule: {}", args.rule)));
    };
    println!("{}\n{}", doc.code, doc.description);
    println!("Applies to: {}\nWhen: {}\nLayout: {}\nRationale: {}", doc.applies_to, doc.when, doc.layout, doc.rationale);
    println!("\nBefore:\n{}\n\nAfter:\n{}", doc.before, doc.after);
    Ok(0)
}

fn load_paths(paths: &[std::path::PathBuf]) -> oh_my_lint_lib::error::Result<Vec<oh_my_lint_lib::source::Document>> {
    let mut documents = Vec::new();
    for path in paths {
        if path.is_dir() {
            collect_directory(path, &mut documents)?;
        } else if path.is_file() {
            let document = oh_my_lint_lib::source::Document::load(path)?;
            if !document.blocks.is_empty() {
                documents.push(document);
            }
        } else {
            return Err(oh_my_lint_lib::error::Error::Rejected(format!("path not found: {}", path.display())));
        }
    }
    documents.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(documents)
}

fn collect_directory(directory: &std::path::Path, documents: &mut Vec<oh_my_lint_lib::source::Document>) -> oh_my_lint_lib::error::Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if path.symlink_metadata().is_ok_and(|metadata| metadata.file_type().is_symlink()) {
                continue;
            }
            if SKIP_DIRECTORIES.iter().any(|skip| name.to_string_lossy() == *skip) {
                continue;
            }
            collect_directory(&path, documents)?;
            continue;
        }
        let extension = path.extension().and_then(|extension| extension.to_str());
        if !matches!(extension, Some("sql" | "py")) {
            continue;
        }
        let document = oh_my_lint_lib::source::Document::load(&path)?;
        if !document.blocks.is_empty() {
            documents.push(document);
        }
    }
    Ok(())
}

fn stdin_path(args: &FixArgs) -> oh_my_lint_lib::error::Result<std::path::PathBuf> {
    if let Some(path) = &args.stdin_filename {
        return Ok(path.clone());
    }
    match args.lang.as_deref() {
        Some("sql") => Ok(std::path::PathBuf::from("stdin.sql")),
        Some(other) => Err(oh_my_lint_lib::error::Error::Unsupported(format!("unsupported language: {other}"))),
        None => Err(oh_my_lint_lib::error::Error::Rejected("--stdin needs --stdin-filename or --lang sql".to_owned())),
    }
}

fn write_atomic(path: &std::path::Path, text: &str) -> oh_my_lint_lib::error::Result<()> {
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let file_name = path.file_name().map_or_else(|| "stdin".into(), |name| name.to_string_lossy());
    let temporary = parent.join(format!(".{file_name}.oh-my-lint.tmp"));
    {
        let mut file = std::fs::File::create(&temporary)?;
        std::io::Write::write_all(&mut file, text.as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&temporary, path)?;
    Ok(())
}

fn display_path(path: &std::path::Path) -> String {
    path.strip_prefix(std::env::current_dir().unwrap_or_default()).unwrap_or(path).display().to_string()
}

fn severity_name(severity: oh_my_lint_lib::model::Severity) -> &'static str {
    match severity {
        oh_my_lint_lib::model::Severity::Error => "error",
        oh_my_lint_lib::model::Severity::Warning => "warning",
    }
}

fn json_error(error: serde_json::Error) -> oh_my_lint_lib::error::Error {
    oh_my_lint_lib::error::Error::Unsupported(format!("json: {error}"))
}
