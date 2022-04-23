use std::io::prelude::*;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use filetime::FileTime;
use file_mode::ModeFile;
use is_executable::IsExecutable;
use pathsearch::find_executable_in_path;
use hyper::{Response, Body};
use hyper::body::HttpBody;
use chrono::prelude::*;

use crate::error::Error;

pub const TIME_FORMAT: &str = "%a, %d %b %Y %H:%M:%S GMT";

pub fn try_make_dir(root: &OsString, dir: &str) -> Result<PathBuf, Error> {
    let mut path = PathBuf::with_capacity(root.len() + 1 + dir.len());
    path.push(root);
    path.push(dir);
    if let Ok(md) = path.as_path().metadata() {
        if !md.is_dir() {
            return Err(Error::PathExists(String::from(path.as_os_str().to_string_lossy())))
        } else {
            return Ok(path)
        }
    }
    std::fs::create_dir(path.as_path())?;
    Ok(path)
}

pub fn find_rsvg(n: &OsStr) -> Result<OsString, Error> {
    let p = Path::new(n);
    if let Some(x) = p.parent() {
        if x.as_os_str() == "" {
            match find_executable_in_path(p) {
                None => Err(Error::NotFound(String::from(n.to_string_lossy()))),
                Some(p) => Ok(OsString::from(p.as_os_str()))
            }
        } else if p.exists() || p.is_executable() {
            Ok(OsString::from(p.as_os_str()))
        } else {
            Err(Error::NotFound(String::from(n.to_string_lossy())))
        }
    } else {
        Err(Error::NotFound(String::from(n.to_string_lossy())))
    }
}

pub async fn fetch_file(dst: &Path, rsp: &mut Response<Body>) -> Result<(), Error> {
    use hyper::header::LAST_MODIFIED;
    let mut tmp = if let Some(dir) = dst.parent() {
        tempfile::NamedTempFile::new_in(dir)
    } else {
        tempfile::NamedTempFile::new_in("./")
    }?;
    let file = tmp.as_file_mut();
    let mut written: usize = 0;
    while let Some(next) = rsp.data().await {
        let chunk = next?;
        if written > 1 * 1024 * 1024 {
            return Err(Error::FileIsTooLarge())
        }
        file.write_all(&chunk)?;
        written += chunk.len();
    }
    if let Some(mt) = rsp.headers().get(LAST_MODIFIED) {
        if let Ok(mt) = mt.to_str() {
            if let Ok(mt) = chrono::Utc.datetime_from_str(mt, TIME_FORMAT) {
                let _ = filetime::set_file_handle_times(
                    file,
                    None,
                    Some(FileTime::from_unix_time(mt.timestamp(), mt.nanosecond()))
                );
            }
        }
    }
    file.set_mode(0o0640)?;
    tmp.persist(dst)?;
    Ok(())
}