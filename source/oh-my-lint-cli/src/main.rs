//! `oh-my-lint` 命令行入口：检查、块级修复、规则列表。

const SKIP_DIRECTORIES: &[&str] = &[
    ".git",
    ".venv",
    "__pycache__",
    ".ruff_cache",
    "build",
    "dist",
    "node_modules",
    "target",
];

/// 主命令。
#[derive(clap::Parser)]
#[command(
    name = "oh-my-lint",
    version,
    about = "按房规检查并修复代码；修复只碰最小语法块"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// 检查路径下的 SQL 块。
    Check(CheckArgs),
    /// 修复 SQL 块；默认 dry-run。
    Fix(FixArgs),
    /// 列出已注册规则。
    Rules(RulesArgs),
    /// 解释一条规则。
    Explain(ExplainArgs),
}

#[derive(clap::Args)]
struct CheckArgs {
    /// 文件或目录；默认当前目录。
    #[arg(default_value = ".")]
    paths: Vec<std::path::PathBuf>,
    /// 输出 JSON。
    #[arg(long)]
    json: bool,
    /// 按规则统计。
    #[arg(long)]
    statistics: bool,
    /// 允许的警告数；超过才失败。
    #[arg(long, default_value_t = 0)]
    max_warnings: usize,
}

#[derive(clap::Args)]
struct FixArgs {
    /// 文件或目录；默认当前目录。
    #[arg(default_value = ".")]
    paths: Vec<std::path::PathBuf>,
    /// 指定单个文件，配合 --range。
    #[arg(long, conflicts_with = "stdin")]
    file: Option<std::path::PathBuf>,
    /// 只修复落在 `L:C-L:C` 范围内的块。
    #[arg(long)]
    range: Option<String>,
    /// 从标准输入读代码。
    #[arg(long, conflicts_with_all = ["file", "write"])]
    stdin: bool,
    /// stdin 的文件名，用于判断语言。
    #[arg(long)]
    stdin_filename: Option<std::path::PathBuf>,
    /// 显式指定语言；目前只支持 sql。
    #[arg(long)]
    lang: Option<String>,
    /// 只看差异，不写回。
    #[arg(long, conflicts_with = "write")]
    #[allow(dead_code)]
    dry_run: bool,
    /// 输出统一 diff。
    #[arg(long)]
    diff: bool,
    /// 写回文件。
    #[arg(long)]
    write: bool,
    /// 输出 JSON。
    #[arg(long)]
    json: bool,
    /// 允许应用 unsafe 修复。
    #[arg(long = "unsafe")]
    unsafe_fixes: bool,
}

#[derive(clap::Args)]
struct RulesArgs {
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct ExplainArgs {
    rule: String,
}

#[derive(serde::Serialize)]
struct FindingOut {
    file: String,
    line: usize,
    column: usize,
    rule: String,
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
    skipped: Vec<SkippedOut>,
    remaining: Vec<FindingOut>,
    written: bool,
}

#[derive(serde::Serialize)]
struct RuleOut {
    id: String,
    description: String,
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
    let engine = oh_my_lint_lib::engine::Engine::with_defaults();
    match cli.command {
        Command::Check(args) => check(&engine, args),
        Command::Fix(args) => fix(&engine, args),
        Command::Rules(args) => rules(&engine, args),
        Command::Explain(args) => explain(&engine, args),
    }
}

fn check(
    engine: &oh_my_lint_lib::engine::Engine,
    args: CheckArgs,
) -> oh_my_lint_lib::error::Result<u8> {
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
                rule: located.finding.rule.clone(),
                severity: severity_name(located.finding.severity).to_owned(),
                message: located.finding.message.clone(),
                fixable: located.fixable,
            });
        }
    }

    if args.statistics {
        let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        for finding in &findings {
            *counts.entry(finding.rule.as_str()).or_default() += 1;
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
        let output = CheckOut {
            findings,
            summary: SummaryOut {
                files: documents.len(),
                blocks,
                findings: total,
            },
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&output).map_err(json_error)?
        );
    } else {
        for finding in &findings {
            println!(
                "{}:{}:{}: {} {}: {}",
                finding.file,
                finding.line,
                finding.column,
                finding.rule,
                finding.severity,
                finding.message
            );
        }
        println!(
            "checked {} files, {} blocks, {} findings",
            documents.len(),
            blocks,
            total
        );
    }

    if total > args.max_warnings {
        Ok(1)
    } else {
        Ok(0)
    }
}

fn fix(
    engine: &oh_my_lint_lib::engine::Engine,
    args: FixArgs,
) -> oh_my_lint_lib::error::Result<u8> {
    if args.stdin {
        return fix_stdin(engine, &args);
    }
    if let Some(file) = &args.file {
        return fix_file(engine, file, args.range.as_deref(), &args);
    }
    if args.range.is_some() {
        return Err(oh_my_lint_lib::error::Error::Rejected(
            "--range requires --file or --stdin".to_owned(),
        ));
    }
    fix_paths(engine, &args)
}

fn fix_stdin(
    engine: &oh_my_lint_lib::engine::Engine,
    args: &FixArgs,
) -> oh_my_lint_lib::error::Result<u8> {
    let mut text = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)?;
    let path = stdin_path(args)?;
    let document = oh_my_lint_lib::source::Document::from_text(path, text)?;
    let range = args
        .range
        .as_deref()
        .map(|spec| document.parse_range(spec))
        .transpose()?;
    let fixes = engine.apply(&document, range, args.unsafe_fixes);
    emit_fixes(&document, &fixes, args, false, true)
}

fn fix_file(
    engine: &oh_my_lint_lib::engine::Engine,
    file: &std::path::Path,
    range: Option<&str>,
    args: &FixArgs,
) -> oh_my_lint_lib::error::Result<u8> {
    let document = oh_my_lint_lib::source::Document::load(file)?;
    let snippet = range.is_some();
    let range = range.map(|spec| document.parse_range(spec)).transpose()?;
    let fixes = engine.apply(&document, range, args.unsafe_fixes);
    emit_fixes(&document, &fixes, args, args.write, snippet)
}

fn fix_paths(
    engine: &oh_my_lint_lib::engine::Engine,
    args: &FixArgs,
) -> oh_my_lint_lib::error::Result<u8> {
    let documents = load_paths(&args.paths)?;
    let mut outcomes = Vec::new();
    for document in &documents {
        let fixes = engine.apply(document, None, args.unsafe_fixes);
        if fixes.is_empty() {
            continue;
        }
        if args.write {
            let text = oh_my_lint_lib::engine::splice(&document.text, &fixes);
            write_atomic(&document.path, &text)?;
        }
        outcomes.extend(outputs(document, &fixes, args.write));
    }
    print_outcomes(&outcomes, args);
    let changed = outcomes.iter().any(|outcome| !outcome.patch.is_empty());
    let remaining: usize = outcomes.iter().map(|outcome| outcome.remaining.len()).sum();
    if remaining > 0 {
        return Ok(1);
    }
    if args.write {
        Ok(0)
    } else {
        Ok(if changed { 1 } else { 0 })
    }
}

fn emit_fixes(
    document: &oh_my_lint_lib::source::Document,
    fixes: &[oh_my_lint_lib::engine::FileFix],
    args: &FixArgs,
    write: bool,
    snippet: bool,
) -> oh_my_lint_lib::error::Result<u8> {
    if write && !fixes.is_empty() {
        let text = oh_my_lint_lib::engine::splice(&document.text, fixes);
        write_atomic(&document.path, &text)?;
    }
    let outputs = outputs(document, fixes, write);
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&outputs).map_err(json_error)?
        );
    } else if args.diff {
        for outcome in &outputs {
            print!("{}", outcome.patch);
        }
    } else if snippet {
        for outcome in &outputs {
            println!("{}", outcome.fixed_text);
        }
    } else {
        print_outcomes(&outputs, args);
    }
    let changed = outputs.iter().any(|outcome| !outcome.patch.is_empty());
    let remaining: usize = outputs.iter().map(|outcome| outcome.remaining.len()).sum();
    let failed = remaining > 0 || (changed && !write);
    Ok(if failed { 1 } else { 0 })
}

fn outputs(
    document: &oh_my_lint_lib::source::Document,
    fixes: &[oh_my_lint_lib::engine::FileFix],
    written: bool,
) -> Vec<FixOut> {
    fixes
        .iter()
        .map(|fix| {
            let fixed_document = oh_my_lint_lib::source::Document {
                path: document.path.clone(),
                text: fix.fixed.clone(),
                blocks: Vec::new(),
            };
            let (start_line, start_column) = document.line_col(fix.block_span.start);
            let (end_line, end_column) = document.line_col(fix.block_span.end);
            FixOut {
                file: display_path(&document.path),
                block_range: format!("{start_line}:{start_column}-{end_line}:{end_column}"),
                fixed_text: fix.fixed.clone(),
                patch: oh_my_lint_lib::diff::unified(
                    &display_path(&document.path),
                    &fix.original,
                    &fix.fixed,
                ),
                applied: fix.applied.clone(),
                skipped: fix
                    .skipped
                    .iter()
                    .map(|skip| SkippedOut {
                        rule: skip.rule.clone(),
                        reason: skip.reason.clone(),
                    })
                    .collect(),
                remaining: fix
                    .remaining
                    .iter()
                    .map(|finding| finding_output(Some(&fixed_document), finding))
                    .collect(),
                written,
            }
        })
        .collect()
}

fn finding_output(
    document: Option<&oh_my_lint_lib::source::Document>,
    finding: &oh_my_lint_lib::model::Finding,
) -> FindingOut {
    let (line, column) = document.map_or((1, 1), |document| document.line_col(finding.span.start));
    FindingOut {
        file: document.map_or_else(
            || "<fixed>".to_owned(),
            |document| display_path(&document.path),
        ),
        line,
        column,
        rule: finding.rule.clone(),
        severity: severity_name(finding.severity).to_owned(),
        message: finding.message.clone(),
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
        println!(
            "{action} {} [{}] applied={:?} skipped={:?}",
            outcome.file,
            outcome.block_range,
            outcome.applied,
            outcome
                .skipped
                .iter()
                .map(|skip| skip.rule.as_str())
                .collect::<Vec<_>>()
        );
    }
    if outcomes.is_empty() {
        println!("no changes");
    }
}

fn rules(
    engine: &oh_my_lint_lib::engine::Engine,
    args: RulesArgs,
) -> oh_my_lint_lib::error::Result<u8> {
    let rules: Vec<RuleOut> = engine
        .rules()
        .map(|rule| RuleOut {
            id: oh_my_lint_lib::model::Rule::id(rule).to_owned(),
            description: oh_my_lint_lib::model::Rule::description(rule).to_owned(),
        })
        .collect();
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&rules).map_err(json_error)?
        );
    } else {
        for rule in rules {
            println!("{:<16} {}", rule.id, rule.description);
        }
    }
    Ok(0)
}

fn explain(
    engine: &oh_my_lint_lib::engine::Engine,
    args: ExplainArgs,
) -> oh_my_lint_lib::error::Result<u8> {
    let Some(rule) = engine
        .rules()
        .find(|rule| oh_my_lint_lib::model::Rule::id(*rule) == args.rule)
    else {
        return Err(oh_my_lint_lib::error::Error::Rejected(format!(
            "unknown rule: {}",
            args.rule
        )));
    };
    println!("{}", oh_my_lint_lib::model::Rule::id(rule));
    println!("{}", oh_my_lint_lib::model::Rule::description(rule));
    Ok(0)
}

fn load_paths(
    paths: &[std::path::PathBuf],
) -> oh_my_lint_lib::error::Result<Vec<oh_my_lint_lib::source::Document>> {
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
            return Err(oh_my_lint_lib::error::Error::Rejected(format!(
                "path not found: {}",
                path.display()
            )));
        }
    }
    documents.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(documents)
}

fn collect_directory(
    directory: &std::path::Path,
    documents: &mut Vec<oh_my_lint_lib::source::Document>,
) -> oh_my_lint_lib::error::Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if path
                .symlink_metadata()
                .is_ok_and(|metadata| metadata.file_type().is_symlink())
            {
                continue;
            }
            if SKIP_DIRECTORIES
                .iter()
                .any(|skip| name.to_string_lossy() == *skip)
            {
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
        Some(other) => Err(oh_my_lint_lib::error::Error::Unsupported(format!(
            "unsupported language: {other}"
        ))),
        None => Err(oh_my_lint_lib::error::Error::Rejected(
            "--stdin needs --stdin-filename or --lang sql".to_owned(),
        )),
    }
}

fn write_atomic(path: &std::path::Path, text: &str) -> oh_my_lint_lib::error::Result<()> {
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let file_name = path
        .file_name()
        .map_or_else(|| "stdin".into(), |name| name.to_string_lossy());
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
    path.strip_prefix(std::env::current_dir().unwrap_or_default())
        .unwrap_or(path)
        .display()
        .to_string()
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
