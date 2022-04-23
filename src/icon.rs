use std::ffi::OsString;
use std::path::{Path, PathBuf};
use file_mode::ModeFile;
use chrono::offset::Utc;
use chrono::DateTime;
use hyper::{Body, Request, Response, Client, StatusCode};
use hyper::header::{IF_MODIFIED_SINCE, CONTENT_LENGTH, CONTENT_TYPE, LAST_MODIFIED};
use hyper::header::HeaderValue;
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;
use tokio::process::Command;
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};
use log::*;

use crate::error::Error;
use crate::util::{TIME_FORMAT, fetch_file};

#[derive(Clone)]
pub struct IconCache {
    pub convert: OsString,
    pub cache:   PathBuf,
    pub url:     String,
    pub client:  Client<HttpsConnector<HttpConnector>>,
}

impl IconCache {
    async fn fetch_svg(&self, src: &Path, dst: &Path) -> Result<(), Error> {
        let url = format!("{}{}", self.url, src.file_name().unwrap().to_string_lossy());
        let mut req = Request::get(&url)
        .body(Body::from(""))
        .expect("request builder");
        if let Ok(md) = dst.metadata() {
            if let Ok(mt) = md.modified() {
                let mt: DateTime<Utc> = mt.into();
                let mt = HeaderValue::try_from(mt.format(TIME_FORMAT).to_string()).unwrap();
                req.headers_mut().insert(IF_MODIFIED_SINCE, mt);
            }
        }
        let mut rsp = self.client.request(req).await?;
        if rsp.status() == StatusCode::OK {
            fetch_file(src, &mut rsp).await.map(|_| {
                debug!("fetched icon {}", &url);
            })
        } else if rsp.status() == StatusCode::NOT_MODIFIED {
            Err(Error::NotModified())
        } else {
            Err(Error::FetchFailure(rsp.status()))
        }
    }

    async fn convert_svg(&self, src: &Path, dst: &Path, res: u32) -> Result<(), Error> {
        let tmp = if let Some(dir) = dst.parent() {
            tempfile::NamedTempFile::new_in(dir)
        } else {
            tempfile::NamedTempFile::new_in("./")
        }?;
        let status = Command::new(self.convert.as_os_str())
        .arg("-a")
        .arg("-w").arg(res.to_string())
        .arg("-b").arg("black")
        .arg("-o").arg(tmp.path())
        .arg(src)
        .stdin(std::process::Stdio::null())
        .status().await?;
        if !status.success() {
            return Err(Error::Failure(status.code().unwrap_or(-1)))
        }
        let src_md = src.metadata()?;
        filetime::set_file_handle_times(tmp.as_file(), None, Some(filetime::FileTime::from_last_modification_time(&src_md)))?;
        tmp.as_file().set_mode(0o0640)?;
        tmp.persist(dst)?;
        Ok(())
    }

    async fn fetch(&self, name: &str, res: u32) -> Result<PathBuf, Error> {
        let src_ = Path::new(&self.cache).join(Path::new(&name)).with_extension("svg");
        let src = src_.as_path();
        let dst_ = Path::new(&self.cache).join(Path::new(&name)).with_extension(format!("{0}.png", res));
        let dst = dst_.as_path();
        match self.fetch_svg(&src, &dst).await {
            Ok(()) => {
                self.convert_svg(&src, &dst, res).await.and(Ok(dst.into()))
            },
            Err(Error::NotModified()) => Ok(dst.into()),
            Err(err) => Err(err),
        }
    }

    pub async fn serve(&self, name: &str, res: u32) -> Result<Response<Body>, Error> {
        let file = self.fetch(name, res).await?;
        let md = file.as_path().metadata()?;
        let mt: DateTime<Utc> = md.modified().unwrap().into();
        let mt = HeaderValue::try_from(mt.format(TIME_FORMAT).to_string()).unwrap();
        let file = File::open(file).await;
        match file {
            Ok(file) => {
                let stream = FramedRead::new(file, BytesCodec::new());
                let body = Body::wrap_stream(stream);
                Ok(Response::builder()
                .header(CONTENT_TYPE, "image/png")
                .header(CONTENT_LENGTH, md.len())
                .header(LAST_MODIFIED, mt)
                .body(body).unwrap())
            },
            Err(err) => {
                Err(Error::IOError(err))
            }
        }
    }
}
