use dialoguer::{theme::ColorfulTheme, Select};
use std::process::Command;

fn main() -> std::io::Result<()> {
    let current_branch_output = exec_command("git rev-parse --abbrev-ref HEAD");
    let current_branch = current_branch_output.trim();

    let all_branches_output = exec_command(
        "git for-each-ref --count=20 --sort=-committerdate refs/heads/ --format='%(refname:short)'",
    );
    let all_branches: Vec<&str> = all_branches_output
        .lines()
        .filter(|s| *s != current_branch)
        .collect();

    let mut options = Vec::new();
    options.push(current_branch);
    options.extend(all_branches);

    let result = Select::with_theme(&ColorfulTheme::default())
        .items(&options)
        .paged(true)
        .default(0)
        .with_prompt("Which branch would you like to switch to?")
        .interact_opt()
        .expect("No selection");

    match result {
        Some(selection) => {
            let selected_branch = options[selection];

            if selected_branch == current_branch {
                println!("Remaining on branch \"{}\"", current_branch);
                return Ok(());
            }

            println!("Switching to branch \"{}\"", selected_branch);
            let checkout_command = format!("git checkout {}", selected_branch);
            spawn_command(&checkout_command);
        }
        None => println!("Remaining on branch \"{}\"", current_branch),
    }

    Ok(())
}

fn exec_command(command: &str) -> String {
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(&["/C", command])
            .output()
            .expect("failed to execute process")
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .expect("failed to execute process")
    };

    String::from_utf8(output.stdout).expect("Can't parse string as utf-8")
}

fn spawn_command(command: &str) {
    let mut child = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(&["/C", command])
            .spawn()
            .expect("failed to execute process")
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(command)
            .spawn()
            .expect("failed to execute process")
    };

    child.wait().expect("Failed to wait on child");
}
