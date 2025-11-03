use crate::{
    ckb::CkbCtx,
    config::AppConfig,
    db::{establish_connection, query_count},
    error::AppError,
    router::{query_did_doc, resolve_ckb_addr},
};
use actix_cors::Cors;
use actix_files::NamedFile;
use actix_web::{
    App, Either, HttpRequest, HttpResponse, HttpServer, Responder, Result,
    http::{Method, StatusCode},
    middleware, web,
};
use ckb_sdk::{CkbRpcAsyncClient, NetworkType};
use ckb_types::H256;
use std::str::FromStr;
use tokio::{select, signal::ctrl_c, task};
use tokio_util::sync::CancellationToken;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[macro_use]
extern crate tracing;

async fn default_handler(req_method: Method) -> Result<impl Responder> {
    match req_method {
        Method::GET => {
            let file = NamedFile::open("static/404.html")?
                .customize()
                .with_status(StatusCode::NOT_FOUND);
            Ok(Either::Left(file))
        }
        _ => Ok(Either::Right(HttpResponse::MethodNotAllowed().finish())),
    }
}

#[actix_web::main]
async fn main() -> Result<(), AppError> {
    let config = AppConfig::from_env();
    let log_level = Level::from_str(&config.log_level).unwrap();
    let subscriber = FmtSubscriber::builder().with_max_level(log_level).finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    info!("Config: {config:?}");

    let pool = establish_connection(config.data_base_url);
    let token = CancellationToken::new();
    let pool_for_rolling = pool.clone();
    let mut conn = pool_for_rolling.get().unwrap();
    let mut ckb_ctx = CkbCtx::init(&mut conn, token).await;

    let task_handle = task::spawn(async move {
        let target_code_hash = H256::from_str(&config.code_hash).unwrap();
        let client = CkbRpcAsyncClient::new(&config.ckb_node);
        let mut is_sync = true;
        let start_height = config.start_height;
        let mut height = match query_count(&mut conn) {
            Ok(count) => {
                if count < start_height as i64 {
                    info!("Use config height: {start_height}");
                    start_height
                } else {
                    info!("Found old count record: {count}");
                    count as u64
                }
            }
            Err(AppError::CountNotFound) => {
                info!("Not found old count record, start from: {start_height}");
                start_height
            }
            Err(e) => return Err(AppError::DbCountError(e.to_string())),
        };
        let select_token = ckb_ctx.token.clone();
        let mut count = 0;
        loop {
            select! {
                _ = select_token.cancelled() => {
                    info!("Async task: Received cancel signal, exiting...");
                    return Err(AppError::RunTimeError(
                        "Async task: Received cancel signal, exiting...".to_string()));
                },
                _ = ctrl_c() => {
                    info!("Async task: Received shutdown signal, exiting...");
                    select_token.cancel();
                    return Err(AppError::RunTimeError(
                        "Async task: Received shutdown signal, exiting...".to_string()));
                },
                res = ckb_ctx.rolling(
                    height,
                    &client,
                    &mut conn,
                    NetworkType::from_raw_str(&config.ckb_network)
                        .expect("Config CKB_NETWORK set 'ckb' or 'ckb_testnet'"),
                    target_code_hash.clone(),
                    is_sync,
                ) => {
                    match res {
                        Ok(rolling_result) => {
                            count = 0;
                            is_sync = rolling_result.is_sync;
                            if rolling_result.got_block {
                                height += 1;
                            }
                        },
                        Err(e) => {
                            if count > 10 {
                                select_token.cancel();
                            }
                            count += 1;
                            error!("rolling error: {}", e.to_string())
                        },
                    }

                }
                else => break
            }
        }
        Ok(())
    });

    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .wrap(middleware::Compress::default())
            .wrap(middleware::Logger::default().log_target("@"))
            .wrap(
                Cors::default()
                    .allowed_methods(vec!["GET"])
                    .supports_credentials()
                    .max_age(3600),
            )
            .service(web::resource("/{did}").route(web::get().to(query_did_doc)))
            .service(
                web::resource("/resolve-ckb-addr/{ckbAddr}").route(web::get().to(resolve_ckb_addr)),
            )
            .service(
                web::resource("/test").to(|req: HttpRequest| match *req.method() {
                    Method::GET => HttpResponse::Ok(),
                    Method::POST => HttpResponse::MethodNotAllowed(),
                    _ => HttpResponse::NotFound(),
                }),
            )
            .default_service(web::to(default_handler))
    })
    .bind(("0.0.0.0", config.listen_port as u16))?
    .workers(config.worker_num as usize)
    .run();

    let server_handle = server.handle();
    let server_task = tokio::spawn(server);

    let res = task_handle.await;
    info!("task return: {res:?}");
    server_handle.stop(true).await;
    let _ = server_task.await;

    Ok(())
}

mod cell_data;
mod ckb;
pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod router;
pub mod schema;
pub mod types;
pub mod util;
