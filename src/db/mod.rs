use crate::ckb::AppMode;
use crate::error::AppError;
use crate::schema::indexer::{
    dao_record::dsl as DaoRecordSchema, did_record::dsl as DidRecordSchema,
    vote_record::dsl as VoteRecordSchema,
};
use diesel::pg::PgConnection;
use diesel::query_dsl::methods::{OrderDsl, SelectDsl};
use diesel::{ExpressionMethods, OptionalExtension, RunQueryDsl, result::Error as DsError};

#[tracing::instrument(skip_all)]
pub fn query_latest_height(conn: &mut PgConnection, modes: &[AppMode]) -> Result<i64, AppError> {
    let did_height = if modes.contains(&AppMode::DID) {
        DidRecordSchema::did_record
            .order(DidRecordSchema::height.desc())
            .select(DidRecordSchema::height)
            .first(conn)
            .optional()
            .map_err(|e| AppError::DbExecuteFailed(e.to_string()))?
            .ok_or(AppError::CountNotFound)?
    } else {
        0
    };

    let vote_height = if modes.contains(&AppMode::VOTE) {
        VoteRecordSchema::vote_record
            .order(VoteRecordSchema::height.desc())
            .select(VoteRecordSchema::height)
            .first(conn)
            .optional()
            .map_err(|e| AppError::DbExecuteFailed(e.to_string()))?
            .ok_or(AppError::CountNotFound)?
    } else {
        0
    };

    let dao_height = if modes.contains(&AppMode::DAO) {
        DaoRecordSchema::dao_record
            .order(DaoRecordSchema::height.desc())
            .select(DaoRecordSchema::height)
            .first(conn)
            .optional()
            .map_err(|e| AppError::DbExecuteFailed(e.to_string()))?
            .ok_or(AppError::CountNotFound)?
    } else {
        0
    };

    info!("did_height: {did_height}, vote_height: {vote_height}, dao_height: {dao_height}");
    Ok(did_height.max(vote_height).max(dao_height))
}

fn handle_db_error(err: DsError, qon: bool) -> AppError {
    match err {
        DsError::NotFound => {
            if qon {
                AppError::DbRecordNotFound
            } else {
                AppError::DbInsOrUpFailed
            }
        }
        err => AppError::DbExecuteFailed(err.to_string()),
    }
}

pub mod dao;
pub mod did;
pub mod pds;
pub mod vote;
