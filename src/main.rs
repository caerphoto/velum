use std::process;
use std::env;
use std::path::PathBuf;
use std::fs;

fn main() {
    let base_key = "VELUM_BASE";
    let base_path = match env::var(base_key) {
        Ok(val) => val,
        Err(_) => {
            println!("Error: {} env var not set", base_key);
            process::exit(1);
        }
    };
    let templ_path: PathBuf = [&base_path, "templates"].iter().collect();

    match fs::metadata(&templ_path) {
        Ok(m) => {
            if m.is_dir() {
                m
            } else {
                println!("Error: {} is not a directory", templ_path.display());
                process::exit(1);
            }
        },
        Err(_) => {
            println!("Error: {} doesn't exist or is not accessible", templ_path.display());
            process::exit(1);
        }
    };


    println!("Looking for templates in {}", templ_path.display());
}
