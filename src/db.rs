use crate::error::AppError;
use crate::models;
use crate::schema::indexer::{
    did_delete_record::dsl as DidDeleteSchema, did_record::dsl as DidRecordSchema,
};
use crate::types::Web5DocumentData;
use crate::util::transfer_time;
use diesel::query_dsl::methods::{FilterDsl, OrderDsl, SelectDsl};
use diesel::{
    ExpressionMethods, OptionalExtension, RunQueryDsl, SelectableHelper, delete, insert_into,
    update,
};
use diesel::{pg::PgConnection, r2d2};

pub type DbPool = r2d2::Pool<r2d2::ConnectionManager<PgConnection>>;

#[tracing::instrument(skip_all)]
pub fn establish_connection(db_url: String) -> DbPool {
    info!("Establishing database connection");
    let manager = r2d2::ConnectionManager::<PgConnection>::new(db_url);
    r2d2::Pool::builder()
        .build(manager)
        .expect("database URL should be valid path to SQLite DB file")
}

#[tracing::instrument(skip_all)]
pub fn query_valid_did_doc(
    conn: &mut PgConnection,
    did: String,
) -> Result<Web5DocumentData, AppError> {
    let record = DidRecordSchema::did_record
        .filter(DidRecordSchema::did.eq(did.clone()))
        .filter(DidRecordSchema::valid.eq(true))
        .select(models::DidRecord::as_select())
        .first(conn)
        .optional()
        .map_err(|e| AppError::DbExecuteFailed(e.to_string()))?
        .ok_or(AppError::DidDocNotFound(did.clone()))?;
    Ok(serde_json::from_str(&record.document).map_err(|_| AppError::DidDocNoData(did))?)
}

#[tracing::instrument(skip_all)]
pub fn query_valid_did_doc_by_index(
    conn: &mut PgConnection,
    tx_hash: String,
    out_index: i32,
) -> Result<models::DidRecord, AppError> {
    DidRecordSchema::did_record
        .filter(DidRecordSchema::txHash.eq(tx_hash.clone()))
        .filter(DidRecordSchema::outIndex.eq(out_index))
        .filter(DidRecordSchema::valid.eq(true))
        .select(models::DidRecord::as_select())
        .first(conn)
        .optional()
        .map_err(|e| AppError::DbExecuteFailed(e.to_string()))?
        .ok_or(AppError::DidDocNotFound(format!(
            "tx hash: {tx_hash}, tx index: {out_index}"
        )))
}

#[tracing::instrument(skip_all)]
pub fn query_valid_index_set(
    conn: &mut PgConnection,
) -> Result<Option<Vec<(String, i32)>>, AppError> {
    DidRecordSchema::did_record
        .filter(DidRecordSchema::valid.eq(true))
        .select((DidRecordSchema::txHash, DidRecordSchema::outIndex))
        .get_results(conn)
        .optional()
        .map_err(|e| AppError::DbExecuteFailed(e.to_string()))
}

#[tracing::instrument(skip_all)]
pub fn query_count(conn: &mut PgConnection) -> Result<i64, AppError> {
    DidRecordSchema::did_record
        .order(DidRecordSchema::height.desc())
        .select(DidRecordSchema::height)
        .first(conn)
        .optional()
        .map_err(|e| AppError::DbExecuteFailed(e.to_string()))?
        .ok_or(AppError::CountNotFound)
}

#[tracing::instrument(skip_all)]
pub fn resolve_valid_handle(
    conn: &mut PgConnection,
    handle: String,
) -> Result<String, AppError> {
    DidRecordSchema::did_record
        .filter(DidRecordSchema::handle.eq(handle.clone()))
        .filter(DidRecordSchema::valid.eq(true))
        .select(DidRecordSchema::did)
        .first(conn)
        .optional()
        .map_err(|e| AppError::DbExecuteFailed(e.to_string()))?
        .ok_or(AppError::HandleNotFound(handle.clone()))
}

#[tracing::instrument(skip_all)]
pub fn insert_record(
    conn: &mut PgConnection,
    did: String,
    handle: String,
    time_stamp: u64,
    ckb_addr: String,
    tx_hash: String,
    out_index: i32,
    block_height: i64,
    doc: Web5DocumentData,
    valid: bool,
) -> Result<(), AppError> {
    let created_at = transfer_time(time_stamp);
    let doc_str = serde_json::to_string(&doc).map_err(|e| AppError::RunTimeError(e.to_string()))?;
    let _: String = insert_into(DidRecordSchema::did_record)
        .values((
            DidRecordSchema::did.eq(did),
            DidRecordSchema::handle.eq(handle),
            DidRecordSchema::createdAt.eq(created_at),
            DidRecordSchema::ckbAddress.eq(ckb_addr),
            DidRecordSchema::document.eq(doc_str),
            DidRecordSchema::txHash.eq(tx_hash),
            DidRecordSchema::outIndex.eq(out_index),
            DidRecordSchema::height.eq(block_height),
            DidRecordSchema::valid.eq(valid),
        ))
        .on_conflict_do_nothing()
        .returning(DidRecordSchema::did)
        .get_result(conn)
        .map_err(|e| AppError::DbExecuteFailed(e.to_string()))?;
    Ok(())
}

#[tracing::instrument(skip_all)]
pub fn update_record(
    conn: &mut PgConnection,
    did: String,
    handle: String,
    time_stamp: u64,
    tx_hash: String,
    out_index: i32,
    block_height: i64,
    doc: Web5DocumentData,
) -> Result<(), AppError> {
    let created_at = transfer_time(time_stamp);
    let doc_str = serde_json::to_string(&doc).map_err(|e| AppError::RunTimeError(e.to_string()))?;
    update(DidRecordSchema::did_record)
        .filter(DidRecordSchema::did.eq(did))
        .set((
            DidRecordSchema::handle.eq(handle),
            DidRecordSchema::createdAt.eq(created_at),
            DidRecordSchema::document.eq(doc_str),
            DidRecordSchema::txHash.eq(tx_hash),
            DidRecordSchema::outIndex.eq(out_index),
            DidRecordSchema::height.eq(block_height),
        ))
        .execute(conn)
        .map_err(|e| AppError::DbExecuteFailed(e.to_string()))?;
    Ok(())
}

#[tracing::instrument(skip_all)]
pub fn delete_record(
    conn: &mut PgConnection,
    did: String,
    handle: String,
    time_stamp: u64,
    ckb_addr: String,
    tx_hash: String,
    in_index: i32,
    block_height: i64,
    doc: String,
) -> Result<(), AppError> {
    delete(DidRecordSchema::did_record)
        .filter(DidRecordSchema::did.eq(did.clone()))
        .execute(conn)
        .map_err(|e| AppError::DbExecuteFailed(e.to_string()))?;

    let deleted_at = transfer_time(time_stamp);
    let _: String = insert_into(DidDeleteSchema::did_delete_record)
        .values((
            DidDeleteSchema::did.eq(did),
            DidDeleteSchema::handle.eq(handle),
            DidDeleteSchema::deletedAt.eq(deleted_at),
            DidDeleteSchema::ckbAddress.eq(ckb_addr),
            DidDeleteSchema::document.eq(doc),
            DidDeleteSchema::txHash.eq(tx_hash),
            DidDeleteSchema::inIndex.eq(in_index),
            DidDeleteSchema::height.eq(block_height),
        ))
        .on_conflict_do_nothing()
        .returning(DidDeleteSchema::did)
        .get_result(conn)
        .map_err(|e| AppError::DbExecuteFailed(e.to_string()))?;
    Ok(())
}
