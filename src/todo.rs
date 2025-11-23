use anyhow::Result;

pub fn todo_list() -> Result<()> {
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
