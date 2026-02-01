mod discovery;

extern crate dprint_formatter;

use clap::Parser;
use fama_common;
use std::fs;

#[derive(Parser)]
#[command(name = "fama")]
#[command(about = "A code formatter for frontend projects", long_about = None)]
struct Cli {
    /// Optional glob pattern (defaults to all files)
    #[arg(default_value = "**/*")]
    pattern: String,

    /// Export EditorConfig
    #[arg(long, short)]
    export: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.export {
        export_editorconfig();
        return Ok(());
    }

    format_files(&cli.pattern)?;
    Ok(())
}

fn format_files(pattern: &str) -> anyhow::Result<()> {
    // Discover files using discovery module
    let files = discovery::discover_files(Some(pattern))
        .map_err(|e| anyhow::anyhow!("Failed to discover files: {}", e))?;

    let mut formatted_count = 0;
    let mut unchanged_count = 0;
    let mut error_count = 0;

    // Format each file using biome-binding
    for file_path in &files {
        let result = format_file(file_path);
        match result {
            Ok(true) => formatted_count += 1,
            Ok(false) => unchanged_count += 1,
            Err(e) => {
                eprintln!("Error: {}", e);
                error_count += 1;
            }
        }
    }

    // Print results
    println!(
        "Formatted {} files, {} unchanged, {} errors",
        formatted_count, unchanged_count, error_count
    );
    Ok(())
}

fn format_file(file_path: &std::path::PathBuf) -> anyhow::Result<bool> {
    // Read file content
    let content = fs::read_to_string(file_path)?;
    let path_str = file_path.to_str().unwrap_or("");

    // Detect file type using fama-common
    let file_type = fama_common::detect_file_type(path_str);

    // Route to appropriate formatter based on file type
    let formatted_content = match file_type {
        // Web files -> biome-web-formatter
        fama_common::FileType::JavaScript
        | fama_common::FileType::TypeScript
        | fama_common::FileType::Jsx
        | fama_common::FileType::Tsx
        | fama_common::FileType::Html
        | fama_common::FileType::Vue
        | fama_common::FileType::Svelte
        | fama_common::FileType::Astro => {
            biome_web_formatter::format_file(&content, path_str, file_type)
                .map_err(|e| anyhow::anyhow!("{}: {}", file_path.display(), e))?
        }
        // Data + Style files -> dprint-formatter
        fama_common::FileType::Yaml
        | fama_common::FileType::Markdown
        | fama_common::FileType::Css
        | fama_common::FileType::Scss
        | fama_common::FileType::Less
        | fama_common::FileType::Sass
        | fama_common::FileType::Dockerfile => {
            dprint_formatter::format_file(&content, path_str, file_type)
                .map_err(|e| anyhow::anyhow!("{}: {}", file_path.display(), e))?
        }
        // Individual language formatters
        fama_common::FileType::Rust => rust_formatter::format_rust(&content, path_str)
            .map_err(|e| anyhow::anyhow!("{}: {}", file_path.display(), e))?,
        fama_common::FileType::Python => ruff_formatter::format_python(&content, path_str)
            .map_err(|e| anyhow::anyhow!("{}: {}", file_path.display(), e))?,
        fama_common::FileType::Kotlin => kotlin_formatter::format_kotlin(&content, path_str)
            .map_err(|e| anyhow::anyhow!("{}: {}", file_path.display(), e))?,
        fama_common::FileType::Lua => lua_formatter::format_lua(&content, path_str)
            .map_err(|e| anyhow::anyhow!("{}: {}", file_path.display(), e))?,
        fama_common::FileType::Shell => sh_formatter::format_shell(&content, path_str)
            .map_err(|e| anyhow::anyhow!("{}: {}", file_path.display(), e))?,
        fama_common::FileType::Unknown => {
            return Err(anyhow::anyhow!(
                "{}: Unknown file type",
                file_path.display()
            ));
        }
    };

    // Only write if content changed
    if formatted_content != content {
        fs::write(file_path, formatted_content)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn export_editorconfig() {
    let editorconfig = r#"root = true

[*]
charset = utf-8
end_of_line = lf
insert_final_newline = true
trim_trailing_whitespace = true
indent_style = space
indent_size = 2
max_line_length = 80

[*.{js,jsx,ts,tsx,mjs,mjsx,mts}]
indent_size = 2

[*.{css,scss,less,sass}]
indent_size = 2

[*.{html,vue,svelte,astro}]
indent_size = 2

[*.{yaml,yml}]
indent_size = 2

[*.{md}]
indent_size = 2

[*.rs]
indent_size = 4
max_line_length = 100

[*.py]
indent_size = 4
max_line_length = 88

[*.{kt,kts}]
indent_size = 4
max_line_length = 120

[*.lua]
indent_size = 2
max_line_length = 120

[*.{sh,bash,zsh}]
indent_size = 4
max_line_length = 80
"#;
    println!("{}", editorconfig);
}
