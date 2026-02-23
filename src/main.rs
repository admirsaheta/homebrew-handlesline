use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Warning,
    Error,
}

#[derive(Debug, Clone)]
struct Diagnostic {
    level: Level,
    message: String,
}

#[derive(Debug, Clone)]
struct Options {
    allow_parent: bool,
}

#[derive(Debug, Clone)]
enum BlockContext {
    Each { alias: String },
}

fn main() {
    let config = match parse_args() {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    let input_text = match read_input(&config) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("Failed to read input: {err}");
            std::process::exit(1);
        }
    };

    let options = Options {
        allow_parent: config.allow_parent,
    };
    let (output, diagnostics) = transpile(&input_text, &options);

    if let Err(err) = write_output(&config, &output) {
        eprintln!("Failed to write output: {err}");
        std::process::exit(1);
    }

    let mut has_error = false;
    for diagnostic in diagnostics {
        match diagnostic.level {
            Level::Warning => eprintln!("warning: {}", diagnostic.message),
            Level::Error => {
                has_error = true;
                eprintln!("error: {}", diagnostic.message);
            }
        }
    }

    if has_error && config.check {
        std::process::exit(1);
    }
}

#[derive(Debug)]
struct Config {
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    stdin: bool,
    allow_parent: bool,
    check: bool,
}

fn parse_args() -> Result<Config, String> {
    let mut input = None;
    let mut output = None;
    let mut stdin = false;
    let mut allow_parent = false;
    let mut check = false;

    let mut args = env::args().skip(1).peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("sline-transpiler {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--" => {
                for remaining in args {
                    if input.is_some() {
                        return Err("Only one input path is supported".to_string());
                    }
                    input = Some(PathBuf::from(remaining));
                }
                break;
            }
            "-o" | "--output" => {
                let value = args.next().ok_or("Missing value for --output")?;
                output = Some(PathBuf::from(value));
            }
            "--stdin" => stdin = true,
            "--allow-parent" => allow_parent = true,
            "--check" => check = true,
            _ if arg.starts_with('-') => return Err(format!("Unknown option: {arg}")),
            _ => {
                if input.is_some() {
                    return Err("Only one input path is supported".to_string());
                }
                input = Some(PathBuf::from(arg));
            }
        }
    }

    if stdin && input.is_some() {
        return Err("Use either --stdin or an input path, not both".to_string());
    }

    if !stdin && input.is_none() {
        return Err("Provide an input path or use --stdin".to_string());
    }

    if let Some(ref path) = input
        && path.is_dir()
    {
        return Err("Directory inputs are not supported yet".to_string());
    }

    Ok(Config {
        input,
        output,
        stdin,
        allow_parent,
        check,
    })
}

fn print_help() {
    let help = r#"sline-transpiler - Handlebars to Sline converter

USAGE:
    sline-transpiler [OPTIONS] <input>
    sline-transpiler [OPTIONS] --stdin

OPTIONS:
    -o, --output <FILE>   Write output to file (default: stdout)
    --stdin               Read input from stdin
    --allow-parent        Strip ../ scope and emit warnings
    --check               Exit with code 1 if errors are found
    -h, --help            Print help
    -V, --version         Print version
"#;
    println!("{help}");
}

fn read_input(config: &Config) -> io::Result<String> {
    if config.stdin {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        Ok(buffer)
    } else if let Some(ref path) = config.input {
        fs::read_to_string(path)
    } else {
        Ok(String::new())
    }
}

fn write_output(config: &Config, output: &str) -> io::Result<()> {
    if let Some(ref path) = config.output {
        fs::write(path, output)
    } else {
        let mut stdout = io::stdout();
        stdout.write_all(output.as_bytes())?;
        stdout.flush()
    }
}

fn transpile(input: &str, options: &Options) -> (String, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let mut output = String::with_capacity(input.len());
    let mut index = 0;
    let mut stack: Vec<BlockContext> = Vec::new();

    while let Some(relative_start) = input[index..].find("{{") {
        let start = index + relative_start;
        output.push_str(&input[index..start]);

        let is_triple = input[start..].starts_with("{{{");
        let open_len = if is_triple { 3 } else { 2 };
        let close_seq = if is_triple { "}}}" } else { "}}" };

        let search_start = start + open_len;
        let close_relative = match input[search_start..].find(close_seq) {
            Some(value) => value,
            None => {
                output.push_str(&input[start..]);
                return (output, diagnostics);
            }
        };
        let end = search_start + close_relative;
        let token_raw = &input[search_start..end];
        let token_trim = token_raw.trim();

        if token_trim.starts_with("!--") {
            output.push_str(&input[start..end + close_seq.len()]);
            index = end + close_seq.len();
            continue;
        }

        if token_trim.starts_with("#comment") {
            if let Some(close_end) = find_block_close(input, end + close_seq.len(), "comment") {
                let inner = &input[end + close_seq.len()..close_end.start];
                output.push_str("{{!--");
                output.push_str(inner);
                output.push_str("--}}");
                index = close_end.end;
                continue;
            } else {
                diagnostics.push(Diagnostic {
                    level: Level::Error,
                    message: "Unclosed {{#comment}} block".to_string(),
                });
                output.push_str(&input[start..end + close_seq.len()]);
                index = end + close_seq.len();
                continue;
            }
        }

        let transformed = transform_tag(token_trim, &mut stack, options, &mut diagnostics);
        if is_triple {
            output.push_str("{{{ ");
            output.push_str(&transformed);
            output.push_str(" }}}");
        } else {
            output.push_str("{{ ");
            output.push_str(&transformed);
            output.push_str(" }}");
        }
        index = end + close_seq.len();
    }

    output.push_str(&input[index..]);

    if !stack.is_empty() {
        for _context in stack {
            diagnostics.push(Diagnostic {
                level: Level::Error,
                message: "Unclosed block: each".to_string(),
            });
        }
    }

    (output, diagnostics)
}

struct BlockClose {
    start: usize,
    end: usize,
}

fn find_block_close(source: &str, start_index: usize, name: &str) -> Option<BlockClose> {
    let mut index = start_index;
    let close_tag = format!("/{}", name);
    while let Some(relative_start) = source[index..].find("{{") {
        let open = index + relative_start;
        let is_triple = source[open..].starts_with("{{{");
        let open_len = if is_triple { 3 } else { 2 };
        let close_seq = if is_triple { "}}}" } else { "}}" };
        let search_start = open + open_len;
        let close_relative = source[search_start..].find(close_seq)?;
        let close = search_start + close_relative;
        let token_raw = &source[search_start..close];
        let token_trim = token_raw.trim();
        if token_trim == close_tag {
            return Some(BlockClose {
                start: open,
                end: close + close_seq.len(),
            });
        }
        index = close + close_seq.len();
    }
    None
}

fn transform_tag(
    tag: &str,
    stack: &mut Vec<BlockContext>,
    options: &Options,
    diagnostics: &mut Vec<Diagnostic>,
) -> String {
    if let Some(rest) = tag.strip_prefix("#each") {
        let (expr, alias) = parse_each(rest.trim());
        stack.push(BlockContext::Each { alias: alias.clone() });
        return format!("#for {} in {}", alias, expr);
    }

    if tag == "/each" {
        match stack.pop() {
            Some(BlockContext::Each { .. }) => {}
            None => diagnostics.push(Diagnostic {
                level: Level::Error,
                message: "Unexpected closing tag /each".to_string(),
            }),
        }
        return "/for".to_string();
    }

    if let Some(rest) = tag.strip_prefix("#unless") {
        let condition = rest.trim();
        return format!("#if !({})", condition);
    }

    if tag == "/unless" {
        return "/if".to_string();
    }

    if let Some(rest) = tag.strip_prefix("#if") {
        let condition = rest.trim();
        return format!("#if {}", condition);
    }

    if tag == "/if" {
        return "/if".to_string();
    }

    if tag == "else" {
        return "else".to_string();
    }

    if tag.starts_with("#with") || tag == "/with" {
        diagnostics.push(Diagnostic {
            level: Level::Warning,
            message: "Handlebars #with blocks are not converted".to_string(),
        });
        return tag.to_string();
    }

    let current_alias = stack
        .iter()
        .rev()
        .map(|context| match context {
            BlockContext::Each { alias } => alias.as_str(),
        })
        .next();

    transform_expression(tag, current_alias, options, diagnostics)
}

fn parse_each(rest: &str) -> (String, String) {
    let marker = " as |";
    if let Some(pos) = rest.find(marker) {
        let expr = rest[..pos].trim();
        let after = &rest[pos + marker.len()..];
        if let Some(end) = after.find('|') {
            let alias = after[..end].trim();
            if !alias.is_empty() {
                return (expr.to_string(), alias.to_string());
            }
        }
    }

    (rest.trim().to_string(), "item".to_string())
}

fn transform_expression(
    tag: &str,
    alias: Option<&str>,
    options: &Options,
    diagnostics: &mut Vec<Diagnostic>,
) -> String {
    let mut content = tag.trim().to_string();

    if content.starts_with("../") {
        if options.allow_parent {
            let mut stripped = content.as_str();
            let mut count = 0;
            while stripped.starts_with("../") {
                stripped = &stripped[3..];
                count += 1;
            }
            diagnostics.push(Diagnostic {
                level: Level::Warning,
                message: format!("Stripped {count} parent scope segments (../)"),
            });
            content = stripped.to_string();
        } else {
            diagnostics.push(Diagnostic {
                level: Level::Error,
                message: "Parent scope access (../) is not supported in Sline".to_string(),
            });
            return tag.to_string();
        }
    }

    if let Some(alias) = alias {
        if content == "this" {
            return alias.to_string();
        }
        if let Some(rest) = content.strip_prefix("this.") {
            return format!("{}.{}", alias, rest);
        }
        if let Some(rest) = content.strip_prefix("./") {
            return format!("{}.{}", alias, rest);
        }
    } else {
        if content == "this" {
            diagnostics.push(Diagnostic {
                level: Level::Warning,
                message: "Found {{this}} without an each context".to_string(),
            });
            return content;
        }
        if let Some(rest) = content.strip_prefix("this.") {
            return rest.to_string();
        }
        if let Some(rest) = content.strip_prefix("./") {
            return rest.to_string();
        }
    }

    content
}
