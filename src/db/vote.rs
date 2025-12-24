use crate::error::AppError;
use crate::models;
use crate::schema::indexer::vote_record::dsl as VoteRecordSchema;
use diesel::pg::PgConnection;
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl, insert_into};

#[tracing::instrument(skip_all)]
pub fn insert_vote_record(
    conn: &mut PgConnection,
    new_vote_record: models::NewVoteRecord,
) -> Result<(), AppError> {
    let _: i64 = insert_into(VoteRecordSchema::vote_record)
        .values(new_vote_record)
        .on_conflict_do_nothing()
        .returning(VoteRecordSchema::height)
        .get_result(conn)
        .map_err(|e| AppError::DbExecuteFailed(e.to_string()))?;
    Ok(())
}

#[tracing::instrument(skip_all)]
pub fn query_vote_records_by_epoch_opt(
    conn: &mut PgConnection,
    args: &str,
    epoch_raw: Option<i64>,
) -> Result<Vec<(String, Vec<Option<i32>>)>, AppError> {
    // check args
    let args_len = args.len();
    if args_len < 40 {
        error!("args({args}) length < 40");
        return Err(AppError::VoteParamsError(format!(
            "args({args}) length < 40"
        )));
    }
    let args = &args[args_len - 40..];
    if let Some(epoch_raw) = epoch_raw {
        if epoch_raw.is_negative() {
            error!("epoch_raw({epoch_raw}) must be positive");
            return Err(AppError::VoteParamsError(format!(
                "epoch_raw({epoch_raw}) must be positive"
            )));
        }
        VoteRecordSchema::vote_record
            .filter(VoteRecordSchema::args.eq(args))
            .filter(VoteRecordSchema::epochRaw.le(epoch_raw))
            .select((VoteRecordSchema::address, VoteRecordSchema::voteIndex))
            .order((
                VoteRecordSchema::address.asc(),
                VoteRecordSchema::epochRaw.desc(),
                VoteRecordSchema::txIndex.desc(),
                VoteRecordSchema::outIndex.desc(),
            ))
            .distinct_on(VoteRecordSchema::address)
            .get_results(conn)
            .map_err(|e| AppError::DbExecuteFailed(e.to_string()))
    } else {
        VoteRecordSchema::vote_record
            .filter(VoteRecordSchema::args.eq(args))
            .select((VoteRecordSchema::address, VoteRecordSchema::voteIndex))
            .order((
                VoteRecordSchema::address.asc(),
                VoteRecordSchema::epochRaw.desc(),
                VoteRecordSchema::txIndex.desc(),
                VoteRecordSchema::outIndex.desc(),
            ))
            .distinct_on(VoteRecordSchema::address)
            .get_results(conn)
            .map_err(|e| AppError::DbExecuteFailed(e.to_string()))
    }
}

#[tracing::instrument(skip_all)]
pub fn query_address_vote_by_epoch_opt(
    conn: &mut PgConnection,
    args: &str,
    address: &str,
    epoch_raw: Option<i64>,
) -> Result<Vec<Option<i32>>, AppError> {
    // check args
    let args_len = args.len();
    if args_len < 40 {
        error!("args({args}) length < 40");
        return Err(AppError::VoteParamsError(format!(
            "args({args}) length < 40"
        )));
    }
    let args = &args[args_len - 40..];
    if let Some(epoch_raw) = epoch_raw {
        if epoch_raw.is_negative() {
            error!("epoch_raw({epoch_raw}) must be positive");
            return Err(AppError::VoteParamsError(format!(
                "epoch_raw({epoch_raw}) must be positive"
            )));
        }
        VoteRecordSchema::vote_record
            .filter(VoteRecordSchema::args.eq(args))
            .filter(VoteRecordSchema::epochRaw.le(epoch_raw))
            .filter(VoteRecordSchema::address.eq(address))
            .select(VoteRecordSchema::voteIndex)
            .order((
                VoteRecordSchema::address.asc(),
                VoteRecordSchema::epochRaw.desc(),
                VoteRecordSchema::txIndex.desc(),
                VoteRecordSchema::outIndex.desc(),
            ))
            .distinct_on(VoteRecordSchema::address)
            .get_result(conn)
            .map_err(|e| AppError::DbExecuteFailed(e.to_string()))
    } else {
        VoteRecordSchema::vote_record
            .filter(VoteRecordSchema::args.eq(args))
            .filter(VoteRecordSchema::address.eq(address))
            .select(VoteRecordSchema::voteIndex)
            .order((
                VoteRecordSchema::address.asc(),
                VoteRecordSchema::epochRaw.desc(),
                VoteRecordSchema::txIndex.desc(),
                VoteRecordSchema::outIndex.desc(),
            ))
            .distinct_on(VoteRecordSchema::address)
            .get_result(conn)
            .map_err(|e| AppError::DbExecuteFailed(e.to_string()))
    }
}

#[tracing::instrument(skip_all)]
pub fn query_epoch_length(conn: &mut PgConnection, epoch_num: i64) -> Result<i64, AppError> {
    VoteRecordSchema::vote_record
        .filter(VoteRecordSchema::epochNum.eq(epoch_num))
        .select(VoteRecordSchema::epochLen)
        .first(conn)
        .optional()
        .map_err(|e| AppError::DbExecuteFailed(e.to_string()))?
        .ok_or(AppError::VoteNotFound)
}
