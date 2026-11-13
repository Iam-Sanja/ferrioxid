//! Startet die projektlokale Binary `fexu-manage`.
//!
//! Der Befehlsname wird geprüft und als einzelnes Argument übergeben.
//! Es gibt keine Shell-Zeichenkette und keinen Plugin-Lader.

use std::process::ExitCode;

use fexu_migrations::is_command_name;

pub fn manage(args: Vec<String>) -> ExitCode {
    if let Err(error) = validate_manage_args(&args) {
        eprintln!("{error}");
        return ExitCode::from(2);
    }
    let project = match crate::optional_project() {
        Ok(Some(project)) => project,
        Ok(None) => {
            eprintln!("No fexu.toml found. Nothing was run.");
            return ExitCode::from(2);
        }
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    let mut command = std::process::Command::new("cargo");
    command
        .arg("run")
        .arg("--quiet")
        .arg("--bin")
        .arg("fexu-manage")
        .arg("--")
        .args(&args)
        .current_dir(project.root);
    match command.status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(error) => {
            eprintln!("Could not start the project command binary: {error}");
            ExitCode::from(1)
        }
    }
}

pub(crate) fn validate_manage_args(args: &[String]) -> Result<(), String> {
    let Some(name) = args.first() else {
        return Ok(());
    };
    if name == "--help" || name == "-h" {
        return Ok(());
    }
    if is_command_name(name) {
        Ok(())
    } else {
        Err(
            "Command names are lowercase identifiers such as seed-shop. Nothing was run."
                .to_owned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::validate_manage_args;

    #[test]
    fn command_names_are_identifiers_before_any_process_starts() {
        assert!(validate_manage_args(&[]).is_ok());
        assert!(validate_manage_args(&[
            "seed-shop".into(),
            "--file".into(),
            "fixtures/catalog.json".into()
        ])
        .is_ok());
        assert!(validate_manage_args(&["--help".into()]).is_ok());
        for name in ["../secret", "seed shop", "Seed", "seed;rm", "-file", ""] {
            assert!(validate_manage_args(&[name.into()]).is_err(), "{name}");
        }
    }
}
