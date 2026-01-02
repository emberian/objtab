//! mdbook-exec: Execute shell commands in markdown code blocks and update output
//!
//! This tool finds code blocks containing shell commands (lines starting with `$`)
//! and re-executes them, replacing the output with fresh results.
//!
//! Uses Docker to run commands in a Linux environment for ELF tooling.

use anyhow::{bail, Context, Result};
use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

const DOCKER_IMAGE: &str = "gcc:14";

#[derive(Parser, Debug)]
#[command(name = "mdbook-exec")]
#[command(about = "Execute shell commands in markdown and update output")]
struct Args {
    /// Source directory containing markdown files
    #[arg(short, long, default_value = "src")]
    src: PathBuf,

    /// Dry run - don't write changes, just show what would change
    #[arg(short = 'n', long)]
    dry_run: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Skip Docker check (for CI environments where Docker is guaranteed)
    #[arg(long)]
    skip_docker_check: bool,
}

struct ShellCommand {
    command: String,
    output_lines: Vec<String>,
}

struct DockerRunner {
    container_id: String,
    verbose: bool,
}

impl DockerRunner {
    fn new(verbose: bool) -> Result<Self> {
        if verbose {
            eprintln!("Starting Docker container with image {}...", DOCKER_IMAGE);
        }

        let output = Command::new("docker")
            .args([
                "run",
                "-d",
                "--rm",
                "-w",
                "/work",
                DOCKER_IMAGE,
                "sleep",
                "3600",
            ])
            .output()
            .context("Failed to start Docker container. Is Docker running?")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Failed to start Docker container: {}", stderr);
        }

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if verbose {
            eprintln!("Container started: {}", &container_id[..12]);
        }

        let runner = Self {
            container_id,
            verbose,
        };

        runner.setup_fixtures()?;

        Ok(runner)
    }

    fn setup_fixtures(&self) -> Result<()> {
        if self.verbose {
            eprintln!("Setting up fixtures in container...");
        }

        let fixtures = r#"
cat > math.c << 'FIXTURE_EOF'
int add(int a, int b) {
    return a + b;
}

int multiply(int a, int b) {
    return a * b;
}
FIXTURE_EOF

cat > main.c << 'FIXTURE_EOF'
extern int add(int, int);
extern int multiply(int, int);

int main() {
    int result = add(2, 3);
    result = multiply(result, 4);
    return result;
}
FIXTURE_EOF

cat > simple.c << 'FIXTURE_EOF'
int main() { return 42; }
FIXTURE_EOF

# For ch04 - demonstrating multiple definition errors
cat > file1.c << 'FIXTURE_EOF'
int foo = 1;
FIXTURE_EOF

cat > file2.c << 'FIXTURE_EOF'
int foo = 2;
FIXTURE_EOF

# Compile object files
gcc -c math.c -o math.o
gcc -c main.c -o main.o
gcc main.o math.o -o program
gcc -c simple.c -o simple.o
gcc simple.c -o simple

# For ch07 - shared library example
gcc -fPIC -shared math.c -o libmath.so

# For ch06 - static linking examples  
cat > hello.c << 'FIXTURE_EOF'
#include <stdio.h>
int main() { printf("Hello, world!\n"); return 0; }
FIXTURE_EOF
gcc hello.c -o hello

# Create symlink for libc.a if it exists (location varies by distro)
LIBC_PATH=$(find /usr/lib -name 'libc.a' 2>/dev/null | head -1)
if [ -n "$LIBC_PATH" ]; then
    ln -sf "$LIBC_PATH" libc.a
fi
"#;

        let output = Command::new("docker")
            .args(["exec", &self.container_id, "sh", "-c", fixtures])
            .output()
            .context("Failed to set up fixtures")?;

        if !output.status.success() && self.verbose {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("Warning: Some fixtures may have failed: {}", stderr);
        }

        Ok(())
    }

    fn execute(&self, cmd: &str) -> Result<String> {
        if self.verbose {
            eprintln!("  Executing in container: {}", cmd);
        }

        let output = Command::new("docker")
            .args(["exec", &self.container_id, "sh", "-c", cmd])
            .output()
            .with_context(|| format!("Failed to execute: {}", cmd))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let result = if stdout.is_empty() && !stderr.is_empty() {
            stderr.to_string()
        } else {
            stdout.to_string()
        };

        Ok(result.trim_end().to_string())
    }
}

impl Drop for DockerRunner {
    fn drop(&mut self) {
        if self.verbose {
            eprintln!("Stopping container {}...", &self.container_id[..12]);
        }
        let _ = Command::new("docker")
            .args(["kill", &self.container_id])
            .output();
    }
}

fn check_docker() -> Result<()> {
    let output = Command::new("docker")
        .args(["info"])
        .output()
        .context("Docker not found. Please install Docker.")?;

    if !output.status.success() {
        bail!("Docker is not running. Please start Docker.");
    }

    Ok(())
}

fn parse_shell_commands(content: &str) -> Vec<ShellCommand> {
    let mut commands = Vec::new();
    let mut current_cmd: Option<String> = None;
    let mut current_output: Vec<String> = Vec::new();

    for line in content.lines() {
        if line.starts_with("$ ") {
            if let Some(cmd) = current_cmd.take() {
                commands.push(ShellCommand {
                    command: cmd,
                    output_lines: std::mem::take(&mut current_output),
                });
            }
            current_cmd = Some(line[2..].to_string());
        } else if line.starts_with("# ") && current_cmd.is_none() {
            continue;
        } else if current_cmd.is_some() {
            current_output.push(line.to_string());
        }
    }

    if let Some(cmd) = current_cmd {
        commands.push(ShellCommand {
            command: cmd,
            output_lines: current_output,
        });
    }

    commands
}

fn is_shell_block(lang: &str, content: &str) -> bool {
    lang == "bash" && content.lines().any(|line| line.starts_with("$ "))
}

fn should_skip_command(cmd: &str) -> bool {
    const SKIP_PATTERNS: &[&str] = &[
        "ldd",
        "/proc/",
        "LD_DEBUG",
        "node ",
        "npm ",
        "cargo ",
        "wasm",
        "emcc",
        "clang --target=wasm32",
        "c++filt",
        "checksec",
        "ltrace",
        "twiggy",
        "bloaty",
        "/lib",
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "LD_BIND_NOW",
        "file ",
        "echo $?",
        "node-gyp",
        "rustup",
    ];

    SKIP_PATTERNS.iter().any(|p| cmd.contains(p))
}

fn process_file(path: &Path, runner: &DockerRunner, verbose: bool, dry_run: bool) -> Result<bool> {
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
            if is_shell_block(&code_block_lang, &code_block_content) {
                let commands = parse_shell_commands(&code_block_content);
                let mut new_block = String::new();

                for cmd in &commands {
                    if should_skip_command(&cmd.command) {
                        new_block.push_str(&format!("$ {}\n", cmd.command));
                        for out_line in &cmd.output_lines {
                            new_block.push_str(out_line);
                            new_block.push('\n');
                        }
                    } else {
                        new_block.push_str(&format!("$ {}\n", cmd.command));

                        match runner.execute(&cmd.command) {
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
                                for out_line in &cmd.output_lines {
                                    new_block.push_str(out_line);
                                    new_block.push('\n');
                                }
                            }
                        }
                    }
                }

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

    if !args.skip_docker_check {
        check_docker()?;
    }

    let runner = DockerRunner::new(args.verbose)?;

    let mut files_modified = 0;
    let mut files_processed = 0;

    for entry in WalkDir::new(&args.src)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
    {
        let path = entry.path();
        files_processed += 1;

        if args.verbose {
            eprintln!("Processing {:?}", path);
        }

        match process_file(path, &runner, args.verbose, args.dry_run) {
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
