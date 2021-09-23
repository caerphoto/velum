use std::process;
use std::env;
use std::path::PathBuf;
use std::fs;

fn path_if_valid(base: &str, dir: &str) -> PathBuf {
    let path: PathBuf = [&base, dir].iter().collect();
    match fs::metadata(&path) {
        Ok(metadata) => {
            if metadata.is_dir() {
                metadata
            } else {
                println!("Error: {} is not a directory", path.display());
                process::exit(1);
            }
        },
        Err(_) => {
            println!("Error: {} doesn't exist or is not accessible", path.display());
            process::exit(1);
        }
    };
    path
}

fn main() {
    let base_key = "VELUM_BASE";
    let base_path = match env::var(base_key) {
        Ok(val) => val,
        Err(_) => {
            println!("Error: {} env var not set", base_key);
            process::exit(1);
        }
    };

    let templ_path = path_if_valid(&base_path, "templates");
    let article_path = path_if_valid(&base_path, "articles");

    println!("Looking for templates in {}", templ_path.display());
    println!("Looking for articles in {}", article_path.display());
}
