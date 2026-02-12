use crate::db::handle_db_error;
use crate::error::AppError;
use crate::models;
use crate::schema::indexer::pds_list::dsl as PdsListSchema;
use diesel::pg::PgConnection;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper, insert_into, update};

#[tracing::instrument(skip_all)]
pub fn query_pds_num(conn: &mut PgConnection) -> Result<i64, AppError> {
    PdsListSchema::pds_list
        .count()
        .get_result(conn)
        .map_err(|e| {
            error!("db operation failed: {}", e.to_string());
            handle_db_error(e, true)
        })
}

#[tracing::instrument(skip_all)]
pub fn query_pds_list(conn: &mut PgConnection) -> Result<Vec<String>, AppError> {
    PdsListSchema::pds_list
        .select(PdsListSchema::pds_url)
        .get_results(conn)
        .map_err(|e| {
            error!("db operation failed: {}", e.to_string());
            handle_db_error(e, true)
        })
}

#[tracing::instrument(skip_all)]
pub fn query_pds(conn: &mut PgConnection, pds_url: &str) -> Result<models::PdsList, AppError> {
    PdsListSchema::pds_list
        .filter(PdsListSchema::pds_url.eq(pds_url))
        .select(models::PdsList::as_select())
        .get_result(conn)
        .map_err(|e| {
            error!("db operation failed: {}", e.to_string());
            handle_db_error(e, true)
        })
}

#[tracing::instrument(skip_all)]
pub fn insert_pds(conn: &mut PgConnection, pds: &models::PdsList) -> Result<(), AppError> {
    let _: i64 = insert_into(PdsListSchema::pds_list)
        .values((
            PdsListSchema::pds_url.eq(&pds.pds_url),
            PdsListSchema::user_num.eq(pds.user_num),
        ))
        .on_conflict_do_nothing()
        .returning(PdsListSchema::user_num)
        .get_result(conn)
        .map_err(|e| {
            error!("db operation failed: {}", e.to_string());
            handle_db_error(e, false)
        })?;
    Ok(())
}

#[tracing::instrument(skip_all)]
pub fn update_pds(conn: &mut PgConnection, pds: &models::PdsList) -> Result<(), AppError> {
    let _: i64 = update(PdsListSchema::pds_list.filter(PdsListSchema::pds_url.eq(&pds.pds_url)))
        .set((PdsListSchema::user_num.eq(pds.user_num),))
        .returning(PdsListSchema::user_num)
        .get_result(conn)
        .map_err(|e| {
            error!("db operation failed: {}", e.to_string());
            handle_db_error(e, false)
        })?;
    Ok(())
}
