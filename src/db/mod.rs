use crate::ckb::AppMode;
use crate::error::AppError;
use crate::schema::indexer::{
    did_record::dsl as DidRecordSchema, vote_record::dsl as VoteRecordSchema,
};
use diesel::pg::PgConnection;
use diesel::query_dsl::methods::{OrderDsl, SelectDsl};
use diesel::{ExpressionMethods, OptionalExtension, RunQueryDsl};

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

    if did_height >= vote_height {
        Ok(did_height)
    } else {
        Ok(vote_height)
    }
}

pub mod did;
pub mod vote;
