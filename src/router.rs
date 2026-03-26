use std::collections::HashMap;

use crate::{
    db::{
        dao::query_valid_dao_records_by_addr,
        did::{
            DbPool, query_all_did_doc_by_ckb_addr, query_ckb_addr_by_did, query_valid_did_doc,
            query_valid_did_set_until_height,
        },
        vote::{query_address_vote_by_epoch_opt, query_vote_records_by_epoch_opt},
    },
    error::AppError,
    util::{DaoSummary, check_did_str, compute_stake_num, extract_core_did, generate_epoch_raw},
};
use actix_web::{
    HttpResponse,
    web::{Data, Json, Path, Query, block},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Deserialize, Debug, Default, IntoParams, ToSchema)]
pub struct QueryVoteRecordParams {
    #[param(example = "vote_args")]
    args: String,
    #[param(example = "ckt1...")]
    ckb_address: Option<String>,
    epoch_number: Option<u64>,
    epoch_index: Option<u64>,
    epoch_length: Option<u64>,
}

#[derive(Deserialize, Debug, Default, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VoteRecords {
    ckb_address: String,
    vote_index: Vec<Option<i32>>,
}

#[derive(Deserialize, Debug, Default, IntoParams, ToSchema)]
pub struct QueryDidSetParams {
    until_height: u64,
}

#[derive(Deserialize, Debug, Default, ToSchema)]
pub struct QueryDaoStakeParams {
    until_height: Option<u64>,
    ckb_list: Vec<String>,
}

#[derive(Deserialize, Debug, Default, IntoParams, ToSchema)]
pub struct QueryDaoTxHistoryParams {
    until_height: Option<u64>,
    ckb_address: String,
}

#[derive(Serialize, ToSchema)]
pub struct ErrorResponse {
    message: String,
}

#[derive(Serialize, ToSchema)]
pub struct DaoStakeResponse {
    #[schema(value_type = Object)]
    stakes: HashMap<String, u64>,
}

#[derive(Serialize, ToSchema)]
pub struct DaoSummaryResponse {
    #[schema(value_type = Object)]
    summary: serde_json::Value,
}

#[derive(Serialize, ToSchema)]
pub struct DidSetResponse {
    #[schema(value_type = Object)]
    dids: HashMap<String, String>,
}

/// Query DID document by DID
#[utoipa::path(
    get,
    path = "/resolve-did/{did}",
    params(
        ("did" = String, Path, description = "DID to resolve")
    ),
    responses(
        (status = 200, description = "DID document found", body = String),
        (status = 404, description = "DID not found", body = ErrorResponse),
        (status = 400, description = "Invalid DID format", body = ErrorResponse),
    )
)]
pub async fn resolve_did(path: Path<String>, pool: Data<DbPool>) -> HttpResponse {
    let did = path.into_inner();
    if !check_did_str(&did) {
        return HttpResponse::from_error(AppError::IncompatibleDid(did));
    }
    let core_did = extract_core_did(&did);
    let mut conn = pool.get().unwrap();
    match block(move || query_ckb_addr_by_did(&mut conn, core_did))
        .await
        .map_err(|e| AppError::RunTimeError(e.to_string()))
    {
        Ok(res) => match res {
            Ok(ckb_addr) => HttpResponse::Ok().body(ckb_addr),
            Err(err) => HttpResponse::from_error(err),
        },
        Err(err) => HttpResponse::from_error(err),
    }
}

/// Query all DIDs associated with a CKB address
#[utoipa::path(
    get,
    path = "/resolve-ckb-addr/{ckbAddr}",
    params(
        ("ckbAddr" = String, Path, description = "CKB address to query")
    ),
    responses(
        (status = 200, description = "DIDs found", body = Vec<String>),
        (status = 404, description = "Address not found", body = ErrorResponse),
    )
)]
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

/// Query DID document
#[utoipa::path(
    get,
    path = "/{did}",
    params(
        ("did" = String, Path, description = "DID to query")
    ),
    responses(
        (status = 200, description = "DID document found", body = String),
        (status = 404, description = "DID not found", body = ErrorResponse),
        (status = 400, description = "Invalid DID format", body = ErrorResponse),
    )
)]
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

/// Query all votes
#[utoipa::path(
    get,
    path = "/all-votes",
    params(QueryVoteRecordParams),
    responses(
        (status = 200, description = "Votes retrieved successfully", body = Vec<VoteRecords>),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
    )
)]
pub async fn query_all_votes(
    pool: Data<DbPool>,
    query: Query<QueryVoteRecordParams>,
) -> HttpResponse {
    let query = query.into_inner();
    info!("[query_all_votes]: query parameters: {query:?}");
    let mut conn = pool.get().unwrap();
    if query.epoch_index.is_some() != query.epoch_number.is_some()
        || query.epoch_index.is_some() != query.epoch_length.is_some()
    {
        return HttpResponse::from_error(AppError::VoteParamsError(format!(
            "epoch_number({:?}), epoch_index({:?}) & epoch_length({:?}) must set all or set none",
            query.epoch_number, query.epoch_index, query.epoch_length
        )));
    }
    if let Some(epoch_num) = query.epoch_number {
        let epoch_raw = match generate_epoch_raw(
            epoch_num,
            query.epoch_index.unwrap(),
            query.epoch_length.unwrap(),
        ) {
            Ok(epoch_raw) => epoch_raw,
            Err(err) => return HttpResponse::from_error(err),
        };
        match query_vote_records_by_epoch_opt(&mut conn, &query.args, Some(epoch_raw as i64)) {
            Ok(votes) => {
                let mut records = vec![];
                for vote in votes {
                    records.push(VoteRecords {
                        ckb_address: vote.0,
                        vote_index: vote.1,
                    });
                }
                HttpResponse::Ok().json(records)
            }
            Err(err) => HttpResponse::from_error(err),
        }
    } else {
        match query_vote_records_by_epoch_opt(&mut conn, &query.args, None) {
            Ok(votes) => {
                let mut records = vec![];
                for vote in votes {
                    records.push(VoteRecords {
                        ckb_address: vote.0,
                        vote_index: vote.1,
                    });
                }
                HttpResponse::Ok().json(records)
            }
            Err(err) => HttpResponse::from_error(err),
        }
    }
}

/// Query address vote
#[utoipa::path(
    get,
    path = "/address-vote",
    params(QueryVoteRecordParams),
    responses(
        (status = 200, description = "Vote retrieved successfully", body = Vec<VoteRecords>),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
    )
)]
pub async fn query_address_vote(
    pool: Data<DbPool>,
    query: Query<QueryVoteRecordParams>,
) -> HttpResponse {
    let query = query.into_inner();
    info!("[query_address_vote]: query parameters: {query:?}");
    let mut conn = pool.get().unwrap();
    if query.epoch_index.is_some() != query.epoch_number.is_some()
        || query.epoch_index.is_some() != query.epoch_length.is_some()
    {
        return HttpResponse::from_error(AppError::VoteParamsError(format!(
            "epoch_number({:?}), epoch_index({:?}) & epoch_length({:?}) must set all or set none",
            query.epoch_number, query.epoch_index, query.epoch_length
        )));
    }
    if let Some(ckb_addr) = query.ckb_address {
        if let Some(epoch_num) = query.epoch_number {
            let epoch_raw = match generate_epoch_raw(
                epoch_num,
                query.epoch_index.unwrap(),
                query.epoch_length.unwrap(),
            ) {
                Ok(epoch_raw) => epoch_raw,
                Err(err) => return HttpResponse::from_error(err),
            };
            match query_address_vote_by_epoch_opt(
                &mut conn,
                &query.args,
                &ckb_addr,
                Some(epoch_raw as i64),
            ) {
                Ok(votes) => HttpResponse::Ok().json(votes),
                Err(err) => HttpResponse::from_error(err),
            }
        } else {
            match query_address_vote_by_epoch_opt(&mut conn, &query.args, &ckb_addr, None) {
                Ok(votes) => HttpResponse::Ok().json(votes),
                Err(err) => HttpResponse::from_error(err),
            }
        }
    } else {
        HttpResponse::from_error(AppError::VoteParamsError(format!(
            "You must set ckb address."
        )))
    }
}

/// Query DID set until height
#[utoipa::path(
    get,
    path = "/did-set",
    params(QueryDidSetParams),
    responses(
        (status = 200, description = "DID set retrieved successfully", body = DidSetResponse),
    )
)]
pub async fn query_did_set_until_height(
    pool: Data<DbPool>,
    query: Query<QueryDidSetParams>,
) -> HttpResponse {
    let query = query.into_inner();
    info!("[query_did_set_since_height]: query parameters: {query:?}");
    let mut conn: diesel::r2d2::PooledConnection<
        diesel::r2d2::ConnectionManager<diesel::PgConnection>,
    > = pool.get().unwrap();
    match query_valid_did_set_until_height(&mut conn, query.until_height as i64) {
        Ok(vec) => {
            let mut res_map = HashMap::new();
            for (did, ckb) in vec {
                res_map.insert(did, ckb);
            }
            HttpResponse::Ok().json(res_map)
        }
        Err(err) => HttpResponse::from_error(err),
    }
}

/// Query DAO stake until height
#[utoipa::path(
    post,
    path = "/dao-stake-set",
    request_body = QueryDaoStakeParams,
    responses(
        (status = 200, description = "DAO stakes retrieved successfully", body = DaoStakeResponse),
        (status = 400, description = "Invalid parameters or overflow", body = ErrorResponse),
        (status = 413, description = "Too many addresses (max 20)", body = ErrorResponse),
    )
)]
pub async fn query_dao_stake_until_height(
    pool: Data<DbPool>,
    query: Json<QueryDaoStakeParams>,
) -> HttpResponse {
    let query = query.into_inner();
    info!("[query_dao_stake_until_height]: query parameters: {query:?}");

    let until_height_i64 = match query.until_height {
        Some(height) => {
            if height > i64::MAX as u64 {
                return HttpResponse::from_error(AppError::RunTimeError(format!(
                    "until_height {} exceeds i64::MAX",
                    height
                )));
            }
            height as i64
        }
        None => i64::MAX,
    };

    let mut conn: diesel::r2d2::PooledConnection<
        diesel::r2d2::ConnectionManager<diesel::PgConnection>,
    > = pool.get().unwrap();
    let mut res_map = HashMap::new();
    let ckb_list = query.ckb_list;
    if ckb_list.len() > 20 {
        return HttpResponse::from_error(AppError::DaoStakeOverLimitError);
    }
    for ckb_addr in ckb_list {
        match query_valid_dao_records_by_addr(&mut conn, &ckb_addr, until_height_i64) {
            Ok(dao_records) => {
                match compute_stake_num(dao_records) {
                    Ok(num) => res_map.insert(ckb_addr, num),
                    Err(err) => {
                        error!(
                            "[query_dao_stake_until_height]: calculate {ckb_addr} error: {}",
                            err.to_string()
                        );
                        return HttpResponse::from_error(AppError::RunTimeError(format!(
                            "calculate {ckb_addr} error: {}",
                            err.to_string()
                        )));
                    }
                };
            }
            Err(err) => return HttpResponse::from_error(err),
        }
    }
    HttpResponse::Ok().json(res_map)
}

/// Query DAO stake history
#[utoipa::path(
    get,
    path = "/dao-stake-history",
    params(QueryDaoTxHistoryParams),
    responses(
        (status = 200, description = "DAO stake history retrieved successfully", body = DaoSummaryResponse),
        (status = 400, description = "Invalid parameters or overflow", body = ErrorResponse),
    )
)]
pub async fn query_dao_stake_history(
    pool: Data<DbPool>,
    query: Query<QueryDaoTxHistoryParams>,
) -> HttpResponse {
    let query = query.into_inner();
    info!("[query_dao_stake_history]: query parameters: {query:?}");

    let until_height_i64 = match query.until_height {
        Some(height) => {
            if height > i64::MAX as u64 {
                return HttpResponse::from_error(AppError::RunTimeError(format!(
                    "until_height {} exceeds i64::MAX",
                    height
                )));
            }
            height as i64
        }
        None => i64::MAX,
    };

    let mut conn: diesel::r2d2::PooledConnection<
        diesel::r2d2::ConnectionManager<diesel::PgConnection>,
    > = pool.get().unwrap();

    match query_valid_dao_records_by_addr(&mut conn, &query.ckb_address, until_height_i64) {
        Ok(dao_records) => match DaoSummary::generate_from_record(&dao_records) {
            Ok(summary) => HttpResponse::Ok().json(summary),
            Err(err) => {
                error!(
                    "[query_dao_stake_history]: generate summary {} error: {}",
                    query.ckb_address,
                    err.to_string()
                );
                HttpResponse::from_error(AppError::RunTimeError(format!(
                    "generate summary {} error: {}",
                    query.ckb_address,
                    err.to_string()
                )))
            }
        },
        Err(err) => HttpResponse::from_error(err),
    }
}
