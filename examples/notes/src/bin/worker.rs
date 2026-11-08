//! Notizen-Worker. Er startet keinen Webserver und wendet keine Migration an.

use ferrioxid::apps::FerrioxidApplication;

fn main() {
    let root = std::env::var("FERRIOXID_PROJECT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().expect("cwd"));
    let project = match ferrioxid::migrations::discover_project(&root) {
        Ok(project) => project,
        Err(error) => {
            eprintln!("{}", error.0);
            std::process::exit(2);
        }
    };
    let mut host = ferrioxid::jobs::WorkerHost::from_project(&project)
        .with_mail(ferrioxid::mail_for(&project));
    if let Err(error) = notes::NotesApp.register_jobs(host.registry_mut()) {
        eprintln!("{} {}", error.code, error.detail);
        std::process::exit(1);
    }
    if let Err(error) = host.run_until_ctrl_c() {
        eprintln!("{} {}", error.code(), error.detail());
        let status = if error.code().starts_with("job.migration")
            || error.code() == "job.disabled"
            || error.code() == "job.database"
        {
            2
        } else {
            1
        };
        std::process::exit(status);
    }
}
