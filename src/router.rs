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
    web::{Data, Path, Query, block},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Default)]
pub struct QueryVoteRecordParams {
    args: String,
    ckb_address: Option<String>,
    epoch_number: Option<u64>,
    epoch_index: Option<u64>,
    epoch_length: Option<u64>,
}

#[derive(Deserialize, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoteRecords {
    ckb_address: String,
    vote_index: Vec<Option<i32>>,
}

#[derive(Deserialize, Debug, Default)]
pub struct QueryDidSetParams {
    until_height: u64,
}

#[derive(Deserialize, Debug, Default)]
pub struct QueryDaoStakeParams {
    until_height: u64,
    ckb_list: String,
}

#[derive(Deserialize, Debug, Default)]
pub struct QueryDaoTxHistoryParams {
    until_height: u64,
    ckb_address: String,
}

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

pub async fn query_dao_stake_until_height(
    pool: Data<DbPool>,
    query: Query<QueryDaoStakeParams>,
) -> HttpResponse {
    let query = query.into_inner();
    info!("[query_dao_stake_until_height]: query parameters: {query:?}");
    let mut conn: diesel::r2d2::PooledConnection<
        diesel::r2d2::ConnectionManager<diesel::PgConnection>,
    > = pool.get().unwrap();
    let mut res_map = HashMap::new();
    let ckb_list: Vec<_> = query.ckb_list.split(",").collect();
    if ckb_list.len() > 20 {
        return HttpResponse::from_error(AppError::DaoStakeOverLimitError);
    }
    for ckb_addr in ckb_list {
        match query_valid_dao_records_by_addr(&mut conn, ckb_addr, query.until_height as i64) {
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

pub async fn query_dao_stake_history(
    pool: Data<DbPool>,
    query: Query<QueryDaoTxHistoryParams>,
) -> HttpResponse {
    let query = query.into_inner();
    info!("[query_dao_stake_history]: query parameters: {query:?}");
    let mut conn: diesel::r2d2::PooledConnection<
        diesel::r2d2::ConnectionManager<diesel::PgConnection>,
    > = pool.get().unwrap();

    match query_valid_dao_records_by_addr(&mut conn, &query.ckb_address, query.until_height as i64)
    {
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
