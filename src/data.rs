use std::path::PathBuf;
use std::time::{SystemTime, Duration};
use hyper::{Uri, Body, Request, Response, Client, StatusCode};
use hyper::header::{CONTENT_LENGTH, CONTENT_TYPE};
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};
use log::*;

use crate::error::Error;
use crate::util::fetch_file;

#[derive(Clone)]
pub struct WeatherCache {
    pub cache:   PathBuf,
    pub url:     Uri,
    pub key:     String,
    pub client:  Client<HttpsConnector<HttpConnector>>,
}

impl WeatherCache {
    async fn fetch_data(&self) -> Result<(), Error> {
        if let Ok(md) = self.cache.metadata() {
            if let Ok(mt) = md.modified() {
                let ts = SystemTime::now();
                if mt >= ts.checked_sub(Duration::new(30 * 60, 0)).unwrap() {
                    return Ok(())
                }
            }
        }
        let req = Request::get(&self.url)
        .header("X-Yandex-API-Key", &self.key)
        .body(Body::from(""))
        .expect("request builder");
        let mut rsp = self.client.request(req).await?;
        if rsp.status() == StatusCode::OK {
            fetch_file(self.cache.as_path(), &mut rsp).await.map(|_| {
                debug!("fetched latest weather data");
            })
        } else {
            Err(Error::FetchFailure(rsp.status()))
        }
    }

    pub async fn serve(&self) -> Result<Response<Body>, Error> {
        self.fetch_data().await?;
        let file = File::open(&self.cache).await;
        let md = self.cache.metadata()?;
        match file {
            Ok(file) => {
                let stream = FramedRead::new(file, BytesCodec::new());
                let body = Body::wrap_stream(stream);
                Ok(Response::builder()
                .header(CONTENT_TYPE, "application/json")
                .header(CONTENT_LENGTH, md.len())
                .body(body).unwrap())
            },
            Err(err) => {
                Err(Error::IOError(err))
            }
        }
    }
}
