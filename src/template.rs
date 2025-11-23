use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};

/// Apply a Boilr template
pub fn apply_boilr_template(template: &str, json_path: &Path, interactive: bool) {
    let json_data = fs::read_to_string(json_path).unwrap_or_else(|_| "{}".into());
    let boilr_path = "boilr";

    let mut cmd = Command::new(boilr_path);
    cmd.arg("template").arg("use").arg(template).arg(".");

    if !interactive {
        cmd.arg("--use-defaults");
        cmd.arg("-d").arg(&json_data);
    }

    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    println!("⚙️ Applying boilr template: {}", template);

    let status = cmd.status().expect("Failed to run boilr");

    if !status.success() {
        eprintln!("❌ Boilr failed with exit code {}", status);
    }
}

/// Ask the user to choose a template interactively
pub fn select_template() -> Option<String> {
    let home_dir = env::var("HOME").ok()?;
    let templates_dir = Path::new(&home_dir).join(".config/boilr/templates");
    let entries = fs::read_dir(&templates_dir).ok()?;

    let templates: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            if e.path().is_dir() {
                e.file_name().to_str().map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect();

    if templates.is_empty() {
        eprintln!("No templates found in {}", templates_dir.display());
        return None;
    }

    println!("Available templates:");
    for (i, t) in templates.iter().enumerate() {
        println!("  {}. {}", i + 1, t);
    }

    print!("Select template: ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).ok()?;
    let trimmed = input.trim();

    if let Ok(index) = trimmed.parse::<usize>() {
        if index > 0 && index <= templates.len() {
            return Some(templates[index - 1].clone());
        }
    }

    templates
        .iter()
        .find(|t| t.eq_ignore_ascii_case(trimmed))
        .cloned()
}
