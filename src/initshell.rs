
pub fn detect_shell() -> String {
    if let Ok(_) = std::env::var("BASH") {
        return "bash".into();
    }
    if let Ok(_) = std::env::var("ZSH_NAME") {
        return "zsh".into();
    }
    std::env::var("SHELL").unwrap_or_default()
        .rsplit('/')
        .next()
        .unwrap_or("bash")
        .to_string()
}


pub fn init_shell(shell: &str) {
    // Initialization code for the shell
    match shell {
        "bash" | "zsh" => {
            let code = "

project() {
    # If no args, just call the CLI
    if [ $# -eq 0 ]; then
        command project
        return
    fi

    local proj_name=\"$1\"
    shift  # Remove the first arg

    local proj_dir=\"$HOME/projects/$proj_name\"

    if [ -d \"$proj_dir\" ]; then
        real_path=$(readlink -f \"$proj_dir\")
        cd \"$real_path\" || return
        # Optionally activate .env if it exists
        if [ -f \".env\" ]; then
            # Using direnv style, or just source it
            set -a
            source \".env\"
            set +a
        fi
        # Print status
        command project list | grep \"^$proj_name\"
    else
        # Not a project dir, pass everything to Rust CLI
        command project \"$proj_name\" \"$@\"
    fi
}


alias todo=\"project todo\"
alias projects=\"cd ~/projects/\"
";
            println!("{}", code);
        }
        "fish" => {
            let code = "

";

            println!("{}", code);
        }
        _ => {
            let code = "
echo Unsupported shell

";                        

            println!("{}", code);
        }
    }
}


