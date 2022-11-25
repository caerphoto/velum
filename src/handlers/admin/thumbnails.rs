use std::{
    cmp::{Ord, Ordering},
    collections::BTreeMap,
    error::Error,
    ffi::OsString,
    fmt,
    path::{Path as OsPath, PathBuf},
};

use futures::executor::block_on;
use image::{
    GenericImageView,
    ImageFormat,
    imageops::{resize, FilterType},
};
use serde::Serialize;
use walkdir::{DirEntry, WalkDir};

use crate::SharedData;

const THUMBNAIL_SUFFIX: &str = "_thumbnail";
const THUMB_SIZE: u32 = 150;

#[derive(Serialize)]
pub struct ThumbsRemaining {
    pub total: usize,
    pub count: usize,
}

#[derive(Clone)]
pub struct NameParts {
    path: PathBuf,
    pub dir: PathBuf,
    pub file_name: OsString,
}

impl NameParts {
    pub fn new<P: AsRef<OsPath>>(path: P) -> Result<Self, ThumbError> {
        let path = path.as_ref();
        match (path.parent(), path.file_name()) {
            (Some(p), Some(f)) => Ok(Self {
                    path: path.into(),
                    dir: p.into(),
                    file_name: f.into(),
                }),
            _ => Err(ThumbError::new(path))
        }
    }
}

#[derive(Debug, PartialEq)]
enum ThumbErrorKind {
    Name,
    File,
    AlreadyExists,
}

#[derive(Debug)]
pub struct ThumbError {
    orig_file_name: String,
    kind: ThumbErrorKind,
    details: Option<image::ImageError>,
}

impl ThumbError {
    fn new<P: AsRef<OsPath>>(path: P) -> Self {
        Self {
            orig_file_name: path.as_ref().to_string_lossy().into(),
            kind: ThumbErrorKind::Name,
            details: None,
        }
    }
    fn exists<P: AsRef<OsPath>>(path: P) -> Self {
        Self {
            orig_file_name: path.as_ref().to_string_lossy().into(),
            kind: ThumbErrorKind::AlreadyExists,
            details: None,
        }
    }
}

impl fmt::Display for ThumbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ThumbErrorKind::Name => write!(f, "thumbnail name error: {}", self.orig_file_name),
            ThumbErrorKind::File => write!(f, "thumbnail file error: {:?}", self.details),
            ThumbErrorKind::AlreadyExists => write!(f, "thumbnail {:?} already exists", self.orig_file_name),
        }
    }
}

impl Error for ThumbError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl std::convert::From<image::ImageError> for ThumbError {
    fn from(error: image::ImageError) -> Self {
        Self {
            orig_file_name: String::new(),
            kind: ThumbErrorKind::File,
            details: Some(error),
        }
    }
}

// specialised struct to use as key in BTreeMap, allowing for custom ordering 
pub struct DirKey(pub PathBuf);
impl DirKey {
    fn new<P: AsRef<OsPath>>(p: P) -> Self {
        Self(p.as_ref().to_path_buf())
    }
}
impl fmt::Display for DirKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_string_lossy())
    }
}
impl PartialEq for DirKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for DirKey {}
impl PartialOrd for DirKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let me: &PathBuf = &(self.0);
        let them: &PathBuf = &(other.0);
 
        Some(them.cmp(me))
    }
}
impl Ord for DirKey {
    fn cmp(&self, other:&Self) -> Ordering {
        let me: &PathBuf = &(self.0);
        let them: &PathBuf = &(other.0);
        them.cmp(me)
    }
}
impl Serialize for DirKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        serializer.serialize_str(&self.0.to_string_lossy())
    }
}

#[derive(Debug, Serialize)]
pub struct ImageListEntry {
    pub thumbnail_file_name: String,
    pub orig_file_name: String,
}

impl ImageListEntry {
    fn new<P: AsRef<OsPath>>(file_name: P) -> Result<Self, ThumbError> {
        let file_name = file_name.as_ref();
        let thumbnail_file_name = Self::thumbnail_file_name(file_name)?
            .to_string_lossy()
            .to_string();
        let orig_file_name = file_name.to_string_lossy().to_string();
        Ok(Self { orig_file_name, thumbnail_file_name })
    }
    pub fn thumbnail_file_name<P: AsRef<OsPath>>(file_name: P) -> Result<PathBuf, ThumbError> {
        let file_name = file_name.as_ref();
        if let (Some(stem), Some(ext)) = (file_name.file_stem(), file_name.extension()) {
            Ok(PathBuf::from(
                stem.to_string_lossy().to_string()
                + THUMBNAIL_SUFFIX
                + "."
                + &ext.to_string_lossy()
            ))
        } else {
            Err(ThumbError::new(file_name))
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ImageListDir {
    pub dir: String,
    pub file_names: Vec<ImageListEntry>,
}

impl ImageListDir {
    fn reslash<P: AsRef<OsPath>>(p: P) -> String {
        let s = p.as_ref().to_string_lossy();
        // These names are for output to HTML
        s.replace('\\', "/")
    }
    pub fn new(parts: &NameParts) -> Result<Self, ThumbError> {
        let entry = ImageListEntry::new(&parts.file_name)?;
        Ok(Self {
            dir: Self::reslash(&parts.dir),
            file_names: vec![entry],
        })
    }
    pub fn push<P: AsRef<OsPath>>(&mut self, file_name: P) -> Result<(), ThumbError> {
        let file_name = file_name.as_ref().to_string_lossy().to_string();
        let entry = ImageListEntry::new(&file_name)?;
        self.file_names.push(entry);
        Ok(())
    }
}

fn generate_thumb_path(parts: &NameParts) -> Result<PathBuf, ThumbError> {
    let thumb_name = ImageListEntry::thumbnail_file_name(&parts.file_name)?;
    let thumb_path = parts.dir.join(&thumb_name);
    if thumb_path.is_file() {
        Err(ThumbError::exists(&parts.path))
    } else {
        Ok(thumb_path)
    }
}

async fn create_thumbnail(parts: NameParts, index: usize, count: usize, data: SharedData) -> Result<(), ThumbError> {
    let progress_val = parts.path.clone();
    let ftsize = THUMB_SIZE as f64;
    let thumb_path = match generate_thumb_path(&parts) {
        Ok(p) => p,
        Err(e) => match e.kind {
            ThumbErrorKind::AlreadyExists => return Ok(()),
            _ => return Err(e),
        }
    };
    let result = match image::open(&parts.path) {
        Ok(img) => {
            let (w, h) = img.dimensions();
            let (w, h) = (w as f64, h as f64);
            let (tw, th) = if w > h {
                (ftsize as u32, (ftsize / (w / h)) as u32)
            } else {
                ((ftsize / (h / w)) as u32, ftsize as u32)
            };
            log::info!("[{}/{}] Creating thumbnail for {:?} ...", index, count, thumb_path);
            let thumb = resize(&img, tw, th, FilterType::Triangle);
            if let Err(e) = thumb.save_with_format(&thumb_path, ImageFormat::Jpeg) {
                log::error!("  ...failed to save thumbnail {:?}: {:?}", thumb_path, e);
                Err(e.into())
            } else {
                log::info!("  ...saved thumbnail {:?}", thumb_path);
                Ok(())
            }
        },
        Err(e) => {
            log::error!(
                "[{}/{}] Failed to open image {:?} for thumbnail generation: {:?}",
                index, count, parts.path, e
            );
            Err(e.into())
        }
    };

    data.write().thumb_progress.remove(&progress_val);

    result
}

fn is_valid_image_file(entry: &DirEntry) -> bool {
    let is_image = entry.path().extension()
        .map(|ext| {
            let ext = ext.to_ascii_lowercase();
            ext == "jpg" || ext == "jpeg" || ext == "png" || ext == "gif"
        })
        .unwrap_or(false);
    let is_thumb = entry.path().file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| {
            stem.ends_with(THUMBNAIL_SUFFIX)
        })
        .unwrap_or(true);


    (is_image || entry.file_type().is_dir()) && !is_thumb
}

fn file_sorter(a: &DirEntry, b: &DirEntry) -> std::cmp::Ordering {
    if let (Ok(ma), Ok(mb)) = (a.metadata(), b.metadata()) {
        if let (Ok(ta), Ok(tb)) = (ma.modified(), mb.modified()) {
            return tb.cmp(&ta) // newest first
        }
    }

    std::cmp::Ordering::Equal
}

pub fn get_image_list(data: &SharedData) -> (BTreeMap<DirKey, ImageListDir>, ThumbsRemaining) {
    let dir = PathBuf::from(&data.read().config.content_dir).join("images");
    let iter = WalkDir::new(dir)
        .sort_by(file_sorter)
        .into_iter()
        .filter_entry(is_valid_image_file);

    let mut thumbnail_futures = Vec::new();
    let mut image_files: Vec<NameParts> = Vec::new();
    for entry in iter {
        match entry {
            Ok(dir_entry) => {
                if dir_entry.file_type().is_dir() { continue; }
                let path = dir_entry.path();
                match NameParts::new(path) {
                    Ok(parts) => image_files.push(parts),
                    Err(e) => log::error!("Failed to create name parts from {:?}: {:?}", path, e),
                }
            },
            Err(e) => log::error!("Unable to read dir entry: {:?}", e),
        }
    }

    let mut existing_thumb_count = 0;
    //let mut filenames: HashMap<String, ImageListDir> = HashMap::new();
    let mut filenames: BTreeMap<DirKey, ImageListDir> = BTreeMap::new();
    let count = image_files.len();
    for (i, parts) in image_files.iter().enumerate() {
        if let Err(e) = generate_thumb_path(parts) {
            if e.kind == ThumbErrorKind::AlreadyExists {
                existing_thumb_count += 1;
            }
        } else {
            data.write().thumb_progress.insert(parts.path.clone());
            thumbnail_futures.push(
                create_thumbnail(parts.clone(), i + 1, count, data.clone())
            );
        }

        let key = DirKey::new(&parts.dir);
        if let Some(ild) = filenames.get_mut(&key) {
            if let Err(e) = ild.push(&parts.file_name) {
                log::error!("Failed to push file name/thumbnail to dirlist: {:?}", e)
            }
        } else {
            match ImageListDir::new(parts) {
                Ok(ild) => { filenames.insert(key, ild); },
                Err(e) => { log::error!("Failed to create new image list dir: {:?}", e); },
            }
        }
    }

    let remaining = image_files.len() - existing_thumb_count;
    data.write().initial_remaining_thumbs += remaining;

    // Generate all thumbnails in a separate thread, which is detached and left to do its thing
    tokio::task::spawn_blocking(move || {
        for f in thumbnail_futures {
            if let Err(e) = block_on(f) {
                log::error!("Failed to block on future: {:?}", e);
            }
        }
    });

    (filenames, ThumbsRemaining { count: remaining, total: remaining })
}

