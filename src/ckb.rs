use crate::{
    cell_data::{Bytes, DidWeb5Data, DidWeb5DataUnion},
    db::{delete_record, insert_record, query_valid_did_doc_by_index, query_valid_index_set},
    error::AppError,
    types::Web5DocumentData,
    util::{calculate_address, calculate_web5_did, check_did_doc},
};
use ckb_jsonrpc_types::BlockNumber;
use ckb_sdk::{CkbRpcAsyncClient, NetworkType};
use ckb_types::H256;
use diesel::PgConnection;
use molecule::prelude::Entity;
use std::{collections::HashSet, str::FromStr, time::Duration};
use tokio::time;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct CkbCtx {
    valid_cells: HashSet<(H256, i32)>,
    pub token: CancellationToken,
}

pub struct RollingResult {
    pub is_sync: bool,
    pub got_block: bool,
}

impl CkbCtx {
    pub fn init(conn: &mut PgConnection, token: CancellationToken) -> Self {
        let mut ctx = CkbCtx {
            valid_cells: HashSet::new(),
            token,
        };
        let live_cells = query_valid_index_set(conn).unwrap();
        if let Some(live_cells) = live_cells {
            info!("Ckb Ctx init. Found {} records", live_cells.len());
            for (str, idx) in live_cells {
                ctx.valid_cells.insert((H256::from_str(&str).unwrap(), idx));
            }
        }
        ctx
    }

    pub async fn rolling(
        &mut self,
        query_height: u64,
        client: &CkbRpcAsyncClient,
        conn: &mut PgConnection,
        network: NetworkType,
        target_code_hash: H256,
        mut is_sync: bool,
    ) -> Result<RollingResult, AppError> {
        trace!("Tracing scanning block #{query_height}");
        let got_block = match client
            .get_block_by_number(BlockNumber::from(query_height))
            .await
            .map_err(|e| AppError::CkbRpcError(e.to_string()))?
        {
            Some(block) => {
                if query_height % 100 == 0 {
                    info!("Scanning block #{query_height}");
                    if !is_sync {
                        let tip_number = client
                            .get_tip_block_number()
                            .await
                            .map_err(|e| AppError::CkbRpcError(e.to_string()))?
                            .value();
                        if tip_number > query_height {
                            is_sync = true;
                        }
                    }
                }
                let header = block.header.inner;
                for tx in block.transactions.into_iter() {
                    for (in_index, input) in tx.inner.inputs.into_iter().enumerate() {
                        let pre_tx_hash = input.previous_output.tx_hash.clone();
                        let pre_index = input.previous_output.index.value() as i32;
                        if self.valid_cells.contains(&(pre_tx_hash.clone(), pre_index)) {
                            let did_record = match query_valid_did_doc_by_index(
                                conn,
                                pre_tx_hash.to_string(),
                                pre_index,
                            ) {
                                Ok(data) => data,
                                Err(app_error) => {
                                    error!(
                                        "query_valid_did_doc_by_index failed: {}",
                                        app_error.to_string()
                                    );
                                    self.token.cancel();
                                    return Err(app_error);
                                }
                            };
                            let tx_hash = tx.hash.clone();
                            match delete_record(
                                conn,
                                did_record.did,
                                did_record.handle,
                                header.timestamp.value(),
                                did_record.ckb_address,
                                tx_hash.to_string(),
                                in_index as i32,
                                query_height as i64,
                                did_record.document,
                            ) {
                                Err(app_err) => {
                                    error!("delete_record failed: {}", app_err.to_string());
                                    continue;
                                }
                                Ok(_) => self.valid_cells.remove(&(pre_tx_hash, pre_index)),
                            };
                        }
                    }

                    for (out_inx, output) in tx.inner.outputs.into_iter().enumerate() {
                        if let Some(type_script) = output.type_ {
                            if type_script.code_hash == target_code_hash {
                                let ckb_addr = calculate_address(&output.lock.into(), network);
                                let tx_hash = tx.hash.clone();
                                let args = type_script.args.as_bytes();
                                info!("Get doc cell args: {}", hex::encode(args));
                                let didoc = match parse_didoc_cell(
                                    tx.inner.outputs_data.get(out_inx).unwrap().as_bytes(),
                                ) {
                                    Ok(didoc) => didoc,
                                    Err(app_err) => {
                                        error!("parse_didoc_cell failed: {}", app_err.to_string());
                                        continue;
                                    }
                                };
                                info!(
                                    "Get did document:\n{}",
                                    serde_json::to_string_pretty(&didoc).unwrap()
                                );
                                let handle = match check_did_doc(&didoc) {
                                    Ok(handle) => handle,
                                    Err(app_err) => {
                                        error!("check_did_doc failed: {}", app_err.to_string());
                                        continue;
                                    }
                                };
                                match insert_record(
                                    conn,
                                    calculate_web5_did(&args[..20]),
                                    handle,
                                    header.timestamp.value(),
                                    ckb_addr.to_string(),
                                    tx_hash.to_string(),
                                    out_inx as i32,
                                    query_height as i64,
                                    didoc,
                                    true,
                                ) {
                                    Err(app_err) => {
                                        error!("insert_record failed: {}", app_err.to_string());
                                        continue;
                                    }
                                    _ => {}
                                }
                                self.valid_cells.insert((tx_hash, out_inx as i32));
                            }
                        }
                    }
                }
                true
            }
            None => {
                if is_sync {
                    let tip_number = client
                        .get_tip_block_number()
                        .await
                        .map_err(|e| AppError::CkbRpcError(e.to_string()))?
                        .value();
                    if tip_number < query_height {
                        is_sync = false;
                    }
                }
                false
            }
        };

        let wait = if is_sync {
            Duration::from_secs(0)
        } else {
            Duration::from_secs(3)
        };
        time::sleep(wait).await;
        Ok(RollingResult { is_sync, got_block })
    }
}

fn parse_didoc_cell(cell_data: &[u8]) -> Result<Web5DocumentData, AppError> {
    let did_data = DidWeb5Data::from_slice(cell_data).unwrap();
    let DidWeb5DataUnion::DidWeb5DataV1(did_data_v1) = did_data.to_enum();
    let did_doc: Bytes = did_data_v1.document();
    serde_ipld_dagcbor::from_slice(&did_doc.raw_data())
        .map_err(|e| AppError::DagCborError(e.to_string()))
}
