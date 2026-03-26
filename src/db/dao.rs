use crate::db::handle_db_error;
use crate::error::AppError;
use crate::models;
use crate::schema::indexer::dao_record::dsl as DaoRecordSchema;
use diesel::pg::PgConnection;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper, insert_into};

#[tracing::instrument(skip_all)]
pub fn insert_dao_record(
    conn: &mut PgConnection,
    new_dao_record: models::NewDaoRecord,
) -> Result<(), AppError> {
    if new_dao_record.in_index.is_none() == new_dao_record.out_index.is_none() {
        return Err(AppError::RunTimeError(
            "dao record's in_index and out_index can't exist both or not".to_string(),
        ));
    }
    let _: i64 = insert_into(DaoRecordSchema::dao_record)
        .values(new_dao_record)
        .on_conflict_do_nothing()
        .returning(DaoRecordSchema::height)
        .get_result(conn)
        .map_err(|e| {
            error!("db operation failed: {}", e.to_string());
            handle_db_error(e, false)
        })?;
    Ok(())
}

#[tracing::instrument(skip_all)]
pub fn query_valid_dao_record_by_output(
    conn: &mut PgConnection,
    tx_hash: &str,
    out_index: i32,
) -> Result<models::DaoRecord, AppError> {
    DaoRecordSchema::dao_record
        .filter(DaoRecordSchema::txHash.eq(&tx_hash))
        .filter(DaoRecordSchema::outIndex.eq(out_index))
        .filter(DaoRecordSchema::valid.eq(true))
        .select(models::DaoRecord::as_select())
        .get_result(conn)
        .map_err(|e| AppError::DbExecuteFailed(e.to_string()))
}

#[tracing::instrument(skip_all)]
pub fn query_valid_dao_records_by_addr(
    conn: &mut PgConnection,
    ckb_addr: &str,
    height: i64,
) -> Result<Vec<models::DaoRecord>, AppError> {
    DaoRecordSchema::dao_record
        .filter(DaoRecordSchema::ckbAddress.eq(ckb_addr))
        .filter(DaoRecordSchema::valid.eq(true))
        .filter(DaoRecordSchema::height.le(height))
        .select(models::DaoRecord::as_select())
        .get_results(conn)
        .map_err(|e| AppError::DbExecuteFailed(e.to_string()))
}
