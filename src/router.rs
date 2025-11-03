use crate::{
    db::{DbPool, query_all_did_doc_by_ckb_addr, query_valid_did_doc},
    error::AppError,
    util::{check_did_str, extract_core_did},
};
use actix_web::{
    HttpResponse,
    web::{Data, Path, block},
};

pub async fn query_did_doc(path: Path<String>, pool: Data<DbPool>) -> HttpResponse {
    let did = path.into_inner();
    let mut conn = pool.get().unwrap();
    if !check_did_str(&did) {
        return HttpResponse::from_error(AppError::IncompatibleDid(did));
    }
    let core_did = extract_core_did(&did);
    match block(move || query_valid_did_doc(&mut conn, core_did))
        .await
        .map_err(|e| AppError::RunTimeError(e.to_string()))
    {
        Ok(res) => match res {
            Ok(doc) => HttpResponse::Ok().json(doc),
            Err(err) => HttpResponse::from_error(err),
        },
        Err(err) => HttpResponse::from_error(err),
    }
}

pub async fn resolve_ckb_addr(path: Path<String>, pool: Data<DbPool>) -> HttpResponse {
    let ckb_addr = path.into_inner();
    let mut conn = pool.get().unwrap();
    match block(move || query_all_did_doc_by_ckb_addr(&mut conn, ckb_addr))
        .await
        .map_err(|e| AppError::RunTimeError(e.to_string()))
    {
        Ok(res) => match res {
            Ok(dids) => HttpResponse::Ok().json(dids),
            Err(err) => HttpResponse::from_error(err),
        },
        Err(err) => HttpResponse::from_error(err),
    }
}
