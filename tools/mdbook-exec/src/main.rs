//! mdbook-exec: Execute shell commands in markdown code blocks and update output
//!
//! This tool finds code blocks containing shell commands (lines starting with `$`)
//! and re-executes them, replacing the output with fresh results.
//!
//! It maintains a fixture system for sample files needed by the commands.

use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(name = "mdbook-exec")]
#[command(about = "Execute shell commands in markdown and update output")]
struct Args {
    /// Source directory containing markdown files
    #[arg(short, long, default_value = "src")]
    src: PathBuf,

    /// Fixtures directory containing sample source files
    #[arg(short, long, default_value = "fixtures")]
    fixtures: PathBuf,

    /// Dry run - don't write changes, just show what would change
    #[arg(short = 'n', long)]
    dry_run: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

/// Represents a single command and its expected output
#[derive(Debug)]
struct ShellCommand {
    command: String,
    output_lines: Vec<String>,
}

/// Fixtures needed for various commands
fn create_fixtures(dir: &Path) -> Result<()> {
    // math.c
    fs::write(
        dir.join("math.c"),
        r#"int add(int a, int b) {
    return a + b;
}

int multiply(int a, int b) {
    return a * b;
}
"#,
    )?;

    // main.c
    fs::write(
        dir.join("main.c"),
        r#"extern int add(int, int);
extern int multiply(int, int);

int main() {
    int result = add(2, 3);
    result = multiply(result, 4);
    return result;
}
"#,
    )?;

    // simple.c
    fs::write(dir.join("simple.c"), "int main() { return 42; }\n")?;

    // utils.c (for static linking chapter)
    fs::write(
        dir.join("utils.c"),
        r#"void used_function(void) { }
void unused_function(void) { }
"#,
    )?;

    // plugin.c (for runtime linking chapter)
    fs::write(
        dir.join("plugin.c"),
        r#"#include <stdio.h>

void plugin_init(void) {
    printf("Plugin initialized!\n");
}
"#,
    )?;

    // plugin_user.c
    fs::write(
        dir.join("plugin_user.c"),
        r#"#include <stdio.h>
#include <dlfcn.h>

int main(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr, "Usage: %s <plugin.so>\n", argv[0]);
        return 1;
    }
    
    void *handle = dlopen(argv[1], RTLD_NOW);
    if (!handle) {
        fprintf(stderr, "Error: %s\n", dlerror());
        return 1;
    }
    
    typedef void (*plugin_init_func)(void);
    plugin_init_func init = dlsym(handle, "plugin_init");
    if (!init) {
        fprintf(stderr, "Error: %s\n", dlerror());
        dlclose(handle);
        return 1;
    }
    
    init();
    dlclose(handle);
    return 0;
}
"#,
    )?;

    Ok(())
}

/// Compile fixture files to object files
fn compile_fixtures(dir: &Path, verbose: bool) -> Result<()> {
    let compile_commands = [
        ("gcc", &["-c", "math.c", "-o", "math.o"][..]),
        ("gcc", &["-c", "main.c", "-o", "main.o"][..]),
        ("gcc", &["main.o", "math.o", "-o", "program"][..]),
        ("gcc", &["-c", "simple.c", "-o", "simple.o"][..]),
        ("gcc", &["simple.c", "-o", "simple"][..]),
    ];

    for (cmd, args) in compile_commands {
        if verbose {
            eprintln!("  Running: {} {}", cmd, args.join(" "));
        }
        let output = Command::new(cmd)
            .args(args)
            .current_dir(dir)
            .output()
            .with_context(|| format!("Failed to run {} {:?}", cmd, args))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if verbose {
                eprintln!("  Warning: {} {:?} failed: {}", cmd, args, stderr);
            }
            // Don't fail - some commands might not be needed
        }
    }

    Ok(())
}

/// Parse a code block and extract shell commands with their outputs
fn parse_shell_commands(content: &str) -> Vec<ShellCommand> {
    let mut commands = Vec::new();
    let mut current_cmd: Option<String> = None;
    let mut current_output: Vec<String> = Vec::new();

    for line in content.lines() {
        if line.starts_with("$ ") {
            // Save previous command if exists
            if let Some(cmd) = current_cmd.take() {
                commands.push(ShellCommand {
                    command: cmd,
                    output_lines: std::mem::take(&mut current_output),
                });
            }
            current_cmd = Some(line[2..].to_string());
        } else if line.starts_with("# ") && current_cmd.is_none() {
            // Comment line before any command, skip
        } else if current_cmd.is_some() {
            // This is output from the current command
            current_output.push(line.to_string());
        }
    }

    // Don't forget the last command
    if let Some(cmd) = current_cmd {
        commands.push(ShellCommand {
            command: cmd,
            output_lines: current_output,
        });
    }

    commands
}

/// Execute a command and return its output
fn execute_command(cmd: &str, workdir: &Path, verbose: bool) -> Result<String> {
    if verbose {
        eprintln!("  Executing: {}", cmd);
    }

    // Handle special cases
    let adjusted_cmd = adjust_command(cmd);

    let output = Command::new("sh")
        .arg("-c")
        .arg(&adjusted_cmd)
        .current_dir(workdir)
        .output()
        .with_context(|| format!("Failed to execute: {}", cmd))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Combine stdout and stderr for commands that output to stderr
    let result = if stdout.is_empty() && !stderr.is_empty() {
        stderr.to_string()
    } else {
        stdout.to_string()
    };

    Ok(result.trim_end().to_string())
}

/// Adjust commands for our environment
fn adjust_command(cmd: &str) -> String {
    let cmd = cmd.to_string();

    // Map /bin/ls to actual location (varies by system)
    let cmd = cmd.replace("/bin/ls", "$(which ls)");

    // Map /usr/bin/python3 to actual location
    let cmd = cmd.replace(
        "/usr/bin/python3",
        "$(which python3 2>/dev/null || echo /usr/bin/python3)",
    );

    cmd
}

/// Check if a code block contains shell commands
fn is_shell_block(lang: &str, content: &str) -> bool {
    // Must be a bash block
    if lang != "bash" {
        return false;
    }

    // Must contain at least one $ command
    content.lines().any(|line| line.starts_with("$ "))
}

/// Commands we should skip (they reference files we can't easily create)
fn should_skip_command(cmd: &str) -> bool {
    // Skip commands that reference system binaries we can't control output of
    let skip_patterns = [
        "ldd",                   // System-specific output
        "/proc/",                // Linux-specific
        "LD_DEBUG",              // Runtime-specific
        "node ",                 // Requires Node.js
        "npm ",                  // Requires npm
        "cargo ",                // Requires Rust project context
        "wasm",                  // Requires WASM toolchain
        "emcc",                  // Requires Emscripten
        "clang --target=wasm32", // Requires WASM target
        "c++filt",               // May not be available
        "checksec",              // May not be available
        "ltrace",                // May not be available
        "twiggy",                // May not be available
        "bloaty",                // May not be available
        "/lib",                  // System paths
        "LD_PRELOAD",            // Runtime-specific
        "LD_LIBRARY_PATH",       // Runtime-specific
        "LD_BIND_NOW",           // Runtime-specific
        "file ",                 // Output varies by system
        "echo $?",               // Depends on previous command
    ];

    skip_patterns.iter().any(|p| cmd.contains(p))
}

/// Process a single markdown file
fn process_file(path: &Path, workdir: &Path, verbose: bool, dry_run: bool) -> Result<bool> {
    let content = fs::read_to_string(path)?;
    let mut new_content = String::new();
    let mut modified = false;
    let mut in_code_block = false;
    let mut code_block_lang = String::new();
    let mut code_block_content = String::new();
    let mut code_block_start = String::new();

    for line in content.lines() {
        if line.starts_with("```") && !in_code_block {
            in_code_block = true;
            code_block_lang = line[3..].trim().to_string();
            code_block_start = line.to_string();
            code_block_content.clear();
        } else if line == "```" && in_code_block {
            // End of code block
            if is_shell_block(&code_block_lang, &code_block_content) {
                // Process this block
                let commands = parse_shell_commands(&code_block_content);
                let mut new_block = String::new();

                for cmd in &commands {
                    if should_skip_command(&cmd.command) {
                        // Keep original output
                        new_block.push_str(&format!("$ {}\n", cmd.command));
                        for out_line in &cmd.output_lines {
                            new_block.push_str(out_line);
                            new_block.push('\n');
                        }
                    } else {
                        // Execute and replace
                        new_block.push_str(&format!("$ {}\n", cmd.command));

                        match execute_command(&cmd.command, workdir, verbose) {
                            Ok(output) => {
                                if !output.is_empty() {
                                    new_block.push_str(&output);
                                    new_block.push('\n');
                                }
                            }
                            Err(e) => {
                                if verbose {
                                    eprintln!("  Warning: Command failed: {} - {}", cmd.command, e);
                                }
                                // Keep original output on failure
                                for out_line in &cmd.output_lines {
                                    new_block.push_str(out_line);
                                    new_block.push('\n');
                                }
                            }
                        }
                    }
                }

                // Check if content changed
                let new_block = new_block.trim_end();
                let old_block = code_block_content.trim_end();
                if new_block != old_block {
                    modified = true;
                    if verbose {
                        eprintln!("  Block modified in {:?}", path);
                    }
                }

                new_content.push_str(&code_block_start);
                new_content.push('\n');
                new_content.push_str(new_block);
                new_content.push('\n');
            } else {
                // Not a shell block, keep as-is
                new_content.push_str(&code_block_start);
                new_content.push('\n');
                new_content.push_str(&code_block_content);
            }
            new_content.push_str("```\n");
            in_code_block = false;
        } else if in_code_block {
            code_block_content.push_str(line);
            code_block_content.push('\n');
        } else {
            new_content.push_str(line);
            new_content.push('\n');
        }
    }

    // Remove trailing newline if original didn't have one
    if !content.ends_with('\n') {
        new_content.pop();
    }

    if modified && !dry_run {
        fs::write(path, &new_content)?;
    }

    Ok(modified)
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Create temp directory for fixtures
    let temp_dir = TempDir::new()?;
    let workdir = temp_dir.path();

    if args.verbose {
        eprintln!("Creating fixtures in {:?}", workdir);
    }

    // Set up fixtures
    create_fixtures(workdir)?;
    compile_fixtures(workdir, args.verbose)?;

    // Process all markdown files
    let mut files_modified = 0;
    let mut files_processed = 0;

    for entry in WalkDir::new(&args.src)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "md"))
    {
        let path = entry.path();
        files_processed += 1;

        if args.verbose {
            eprintln!("Processing {:?}", path);
        }

        match process_file(path, workdir, args.verbose, args.dry_run) {
            Ok(modified) => {
                if modified {
                    files_modified += 1;
                    if args.dry_run {
                        println!("Would modify: {:?}", path);
                    } else {
                        println!("Modified: {:?}", path);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error processing {:?}: {}", path, e);
            }
        }
    }

    println!(
        "\nProcessed {} files, {} {}",
        files_processed,
        files_modified,
        if args.dry_run {
            "would be modified"
        } else {
            "modified"
        }
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_shell_commands() {
        let content = r#"$ nm main.o
                 U add
0000000000000000 T main
                 U multiply
"#;
        let commands = parse_shell_commands(content);
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "nm main.o");
        assert_eq!(commands[0].output_lines.len(), 3);
    }

    #[test]
    fn test_parse_multiple_commands() {
        let content = r#"$ echo hello
hello
$ echo world
world
"#;
        let commands = parse_shell_commands(content);
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].command, "echo hello");
        assert_eq!(commands[1].command, "echo world");
    }

    #[test]
    fn test_is_shell_block() {
        assert!(is_shell_block("bash", "$ echo hello\nhello"));
        assert!(!is_shell_block("bash", "echo hello\nhello"));
        assert!(!is_shell_block("c", "$ echo hello"));
    }
}
