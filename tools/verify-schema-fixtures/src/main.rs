use std::env;
use std::path::PathBuf;

fn main() {
    match run() {
        Ok(message) => println!("{message}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<String, String> {
    let root = parse_root(env::args().skip(1).collect())?;
    let report =
        lessonforge_schema::verify_fixture_set(&root).map_err(|error| error.to_string())?;
    Ok(format!(
        "schema fixtures verified: {} fixtures, {} schemas",
        report.checked_fixture_count,
        report.checked_schemas.len()
    ))
}

fn parse_root(args: Vec<String>) -> Result<PathBuf, String> {
    match args.as_slice() {
        [] => env::current_dir().map_err(|error| error.to_string()),
        [flag, root] if flag == "--root" => Ok(PathBuf::from(root)),
        _ => Err("usage: verify-schema-fixtures [--root PATH]".to_owned()),
    }
}
