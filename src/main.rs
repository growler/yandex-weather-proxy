use clap::Parser;
use log::*;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::ffi::OsString;
use hyper::{Uri, Body, Request, Response, Client, Server, StatusCode};
use hyper::service::{make_service_fn, service_fn};
use hyper::server::conn::AddrStream;

mod error;
use crate::error::Error;
mod util;
use crate::util::*;
mod icon;
use crate::icon::*;
mod data;
use crate::data::*;

/// A simple proxy for Yandex weather service
#[derive(Parser, Debug, Clone)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// address and port to listen (use :: or 0.0.0.0 for any)
    #[clap(long, short, parse(try_from_str))]
    listen: SocketAddr,
    /// path to cache directory
    #[clap(long, short, default_value = ".")]
    cache:   OsString,
    /// latitude (`degrees')
    #[clap(long, default_value = "54.19479250963906")]
    lat:    f64,
    /// longitude (`degrees')
    #[clap(long, default_value = "37.61983228236299")]
    lon:    f64,
    /// Yandex API key
    #[clap(long, short, env = "API_KEY")]
    key:    Option<String>,
    /// response language
    #[clap(long, default_value = "ru_RU")]
    lang:   String,
    /// path to rsvg-convert executable
    #[clap(long = "convert", default_value = "rsvg-convert")]
    rsvg:   OsString,
    /// icons style
    #[clap(long, default_value = "funky/dark")]
    style:  String,
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
}

#[derive(Clone)]
struct Context {
    icons:      IconCache,
    weather:    WeatherCache,
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install shutdown handler");
}

fn not_found() -> Result<Response<Body>, Error> {
    Ok(Response::builder().status(StatusCode::NOT_FOUND).body(Body::from("Not found")).unwrap())
}

fn server_error() -> Result<Response<Body>, Error> {
    Ok(Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::from("Internal server error")).unwrap())
}

async fn handle(ctx: Context, addr: SocketAddr, req: Request<Body>) -> Result<Response<Body>, Error> {
    debug!("{}: {} {}", addr, req.method(), req.uri().path());
    if req.method() != hyper::Method::GET {
        error!("unexpected method {} {}", req.method(), req.uri());
        return not_found()
    }
    let parts: Vec<&str> = req.uri().path().split('/').collect();
    if parts.len() == 2 && parts[1] == "weather.json" {
        ctx.weather.serve().await.or_else(|err| {
            error!("error processing request {}: {}", req.uri(), err);
            server_error()
        })
    } else if parts.len() == 4 && parts[1] == "icon" {
        if let Ok(res) = parts[2].parse::<u32>() {
            ctx.icons.serve(parts[3], res).await.or_else(|err| {
                error!("error processing request {}: {}", req.uri(), err);
                server_error()
            })
        } else {
            error!("failed to parse resolution {}", req.uri());
            not_found()
        }
    } else {
        error!("unexpected request {}", req.uri());
        not_found()
    }
}

async fn serve(args: Args) -> Result<(), Error> {

    if args.key == None {
        return Err(Error::NoAPIKey)
    }

    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_only()
        .enable_http1()
        .build();

    let client = Client::builder().build(https);

    let convert = find_rsvg(args.rsvg.as_os_str())?;

    let icons_cache_path = try_make_dir(&args.cache, "icon")?;

    let data_cache_path = try_make_dir(&args.cache, "data")?;

    let ctx = Context{
        icons:  IconCache{
            client:  client.clone(),
            cache:   icons_cache_path,
            convert: convert,
            url:     format!("https://yastatic.net/weather/i/icons/{}/", args.style),
        },
        weather: WeatherCache{
            cache:   data_cache_path.join("data.json"),
            client:  client,
            url:    format!(
                "https://api.weather.yandex.ru/v2/informers?lat={}&lon={}&lang={}",
                args.lat, args.lon, args.lang
            ).parse::<Uri>().expect("API URL parsed"),
            key:    args.key.unwrap(),
        }
    };

    let make_service = make_service_fn(move |conn: &AddrStream| {
        let ctx = ctx.clone();
        let addr = conn.remote_addr();
        let service = service_fn(move |req| {
            handle(ctx.clone(), addr, req)
        });
        async move { Ok::<_, Infallible>(service) }
    });

    let server = Server::try_bind(&args.listen)?.serve(make_service);

    let graceful = server.with_graceful_shutdown(shutdown_signal());

    info!("server started");

    graceful.await?;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let args = Args::parse();

    env_logger::Builder::new()
    .filter_module(module_path!(), args.verbose.log_level_filter())
    .format_module_path(false)
    .format_indent(None)
    .format_timestamp(None)
    .init();

    serve(args).await.map_err(|err| {
        error!("{}", err);
        err
    })
}
