

use anyhow::Result;
use std::fs;
use serde_json::{Value, json};
use std::path::{Path};
use std::fs::File;
use std::io::Write;
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize)]
pub struct Todo {
    title: String,
    description: String,
    complete: bool
}


pub fn read_json(path: &Path) -> Value {
    if let Ok(content) = fs::read_to_string(path) {
        serde_json::from_str(&content).unwrap_or(json!({}))
    } else {
        json!({})
    }
}

pub fn todo_list() -> Result<()> {
    let project_config = dirs::home_dir().unwrap().join(".config/project/");
    let todos_file = project_config.join("todos.json");
    let proj_file = Path::new(&todos_file);
    
    if !project_config.exists(){
        fs::create_dir_all(&project_config)?;
    }

    let todos_file = project_config.join("todos.json");
    if !todos_file.exists(){
        let mut f = File::create(todos_file)?;
        f.write_all(b"{\"todos\":[\"Configure Project Todos\"]}")?;


    }
    
    if let Ok(content) = fs::read_to_string(proj_file) {
        println!("{}", content)
    } else {
        println!("Fuck")
    }
    


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
