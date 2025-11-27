mod utils


use anyhow::Result;
use std::fs;

use std::path::{Path, PathBuf};



pub fn todo_list() -> Result<()> {
    let proj_file = Path::new("~/.config/project/");
    let data = utils::read_json(proj_file);

    println!("List todos");
    Ok(())
}

pub fn todo_add(text: &str) -> Result<()> {
    println!("Add todo: {}", text);
    Ok(())
}

pub fn todo_remove(pattern: &str) -> Result<()> {
    println!("Remove todo: {}", pattern);
    Ok(())
}
