use std::path::Path;
use walkdir::WalkDir;

fn should_skip(path: &Path, matching_ext: &str) -> bool {
    if path.is_dir() {
        return true;
    }
    let ext = path.extension().map(|e| e.to_ascii_lowercase());
    ext.is_none() || ext.unwrap() != matching_ext
}

pub fn paths_with_ext_in_dir<F>(matching_ext: &str, dir: &Path, mut f: F)
where
    F: FnMut(&Path),
{
    for entry in WalkDir::new(dir) {
        let path = entry.expect("Invalid entry").into_path();
        if should_skip(&path, matching_ext) {
            continue;
        }
        f(&path);
    }
}
