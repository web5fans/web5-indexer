use crate::{
    db::{
        did::{
            check_connection, delete_record, insert_record, query_valid_did_doc_by_index,
            query_valid_index_set,
        },
        vote::insert_vote_record,
    },
    error::AppError,
    models,
    molecules::did_cell::{Bytes, DidWeb5Data, DidWeb5DataUnion},
    types::Web5DocumentData,
    util::{
        calculate_address, calculate_web5_did, check_did_doc, generate_epoch_raw, transfer_time,
    },
};
use ckb_jsonrpc_types::{
    AsEpochNumberWithFraction, BlockNumber, CellInput, CellOutput, EpochNumberWithFraction,
    JsonBytes,
};
use ckb_sdk::{CkbRpcAsyncClient, NetworkType};
use ckb_types::H256;
use diesel::PgConnection;
use molecule::prelude::Entity;
use std::{collections::HashSet, str::FromStr, time::Duration};
use tokio::time;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AppMode {
    DID,
    VOTE,
}

impl From<&str> for AppMode {
    fn from(value: &str) -> Self {
        match value {
            "did" | "DID" => Self::DID,
            "vote" | "VOTE" => Self::VOTE,
            _ => {
                panic!("The mode only can be did or vote")
            }
        }
    }
}

#[derive(Default)]
pub struct CkbCtx {
    valid_cells: HashSet<(H256, i32)>,
    pub token: CancellationToken,
    pub mode: Vec<AppMode>,
}

pub struct RollingResult {
    pub is_sync: bool,
    pub got_block: bool,
}

impl CkbCtx {
    pub async fn init(
        conn: &mut PgConnection,
        token: CancellationToken,
        mode: Vec<AppMode>,
    ) -> Self {
        loop {
            if check_connection(conn) {
                break;
            } else {
                info!("Please create indexer schema");
                time::sleep(Duration::from_secs(3)).await;
            }
        }
        if mode.len() == 0 {
            panic!("App mode must be set, support \"vote\" & \"did\".");
        }
        let mut ctx = CkbCtx {
            valid_cells: HashSet::new(),
            token,
            mode,
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
        did_code_hash: &H256,
        vote_code_hash: &H256,
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
                for (tx_index, tx) in block.transactions.into_iter().enumerate() {
                    for (in_index, input) in tx.inner.inputs.into_iter().enumerate() {
                        if self.mode.contains(&AppMode::DID) {
                            self.did_input_handle(
                                conn,
                                in_index as i32,
                                &input,
                                header.timestamp.value(),
                                tx.hash.to_string(),
                                query_height as i64,
                            )?;
                        }
                    }

                    for (out_inx, output) in tx.inner.outputs.into_iter().enumerate() {
                        if self.mode.contains(&AppMode::DID) {
                            self.did_output_handle(
                                conn,
                                &did_code_hash,
                                &tx.inner.outputs_data,
                                out_inx as i32,
                                &output,
                                header.timestamp.value(),
                                tx.hash.clone(),
                                query_height as i64,
                                network,
                            )?;
                        }
                        if self.mode.contains(&AppMode::VOTE) {
                            self.vote_output_handle(
                                conn,
                                &vote_code_hash,
                                &tx.inner.outputs_data,
                                out_inx as i32,
                                &output,
                                header.epoch,
                                header.timestamp.value(),
                                tx_index as i32,
                                tx.hash.clone(),
                                query_height as i64,
                                network,
                            )?;
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

    pub fn did_input_handle(
        &mut self,
        conn: &mut PgConnection,
        in_index: i32,
        input: &CellInput,
        time_stamp: u64,
        tx_hash: String,
        block_height: i64,
    ) -> Result<(), AppError> {
        let pre_tx_hash = input.previous_output.tx_hash.clone();
        let pre_index = input.previous_output.index.value() as i32;
        if self.valid_cells.contains(&(pre_tx_hash.clone(), pre_index)) {
            let did_record =
                match query_valid_did_doc_by_index(conn, pre_tx_hash.to_string(), pre_index) {
                    Ok(data) => data,
                    Err(app_error) => {
                        error!(
                            "[did]: query_valid_did_doc_by_index failed: {}",
                            app_error.to_string()
                        );
                        self.token.cancel();
                        return Err(app_error);
                    }
                };
            match delete_record(
                conn,
                did_record.did,
                did_record.handle,
                did_record.signing_key,
                time_stamp,
                did_record.ckb_address,
                tx_hash,
                in_index,
                block_height,
                did_record.document,
            ) {
                Err(app_err) => {
                    error!("[did]: delete_record failed: {}", app_err.to_string());
                }
                Ok(_) => {
                    self.valid_cells.remove(&(pre_tx_hash, pre_index));
                }
            };
        }
        Ok(())
    }

    pub fn did_output_handle(
        &mut self,
        conn: &mut PgConnection,
        did_code_hash: &H256,
        outputs_data: &Vec<JsonBytes>,
        out_inx: i32,
        output: &CellOutput,
        time_stamp: u64,
        tx_hash: H256,
        block_height: i64,
        network: NetworkType,
    ) -> Result<(), AppError> {
        if let Some(ref type_script) = output.type_ {
            if &type_script.code_hash == did_code_hash {
                let ckb_addr = calculate_address(&output.lock.clone().into(), network);
                let args = type_script.args.as_bytes();
                info!("[did]: Get doc cell args: {}", hex::encode(args));
                let cell_data = match outputs_data.get(out_inx as usize) {
                    Some(out_data) => out_data.as_bytes(),
                    None => {
                        error!(
                            "[did]: tx({}) index({}) out data not found",
                            tx_hash.to_string(),
                            out_inx
                        );
                        return Ok(());
                    }
                };
                let didoc = match parse_didoc_cell(cell_data) {
                    Ok(didoc) => didoc,
                    Err(app_err) => {
                        error!("[did]: parse_didoc_cell failed: {}", app_err.to_string());
                        return Ok(());
                    }
                };
                info!(
                    "[did]: Get did document:\n{}",
                    serde_json::to_string_pretty(&didoc).unwrap()
                );
                let (handle, signing_key) = match check_did_doc(&didoc) {
                    Ok(handle) => handle,
                    Err(app_err) => {
                        error!("[did]: check_did_doc failed: {}", app_err.to_string());
                        return Ok(());
                    }
                };
                match insert_record(
                    conn,
                    calculate_web5_did(&args[..20]),
                    handle,
                    signing_key,
                    time_stamp,
                    ckb_addr.to_string(),
                    tx_hash.to_string(),
                    out_inx,
                    block_height,
                    didoc,
                    true,
                ) {
                    Err(app_err) => {
                        error!("[did]: insert_record failed: {}", app_err.to_string());
                        return Ok(());
                    }
                    _ => {}
                }
                self.valid_cells.insert((tx_hash, out_inx as i32));
            }
        }
        Ok(())
    }

    pub fn vote_output_handle(
        &mut self,
        conn: &mut PgConnection,
        vote_code_hash: &H256,
        outputs_data: &Vec<JsonBytes>,
        out_index: i32,
        output: &CellOutput,
        epoch_raw_data: EpochNumberWithFraction,
        timestamp: u64,
        tx_index: i32,
        tx_hash: H256,
        height: i64,
        network: NetworkType,
    ) -> Result<(), AppError> {
        if let Some(ref type_script) = output.type_ {
            if &type_script.code_hash == vote_code_hash {
                let ckb_addr = calculate_address(&output.lock.clone().into(), network);
                let args = hex::encode(type_script.args.as_bytes());
                info!("[vote]: Get vote cell args: {args}");
                match outputs_data.get(out_index as usize) {
                    Some(out_data) => {
                        let cell_data = out_data.as_bytes();
                        let mut bs = String::new();
                        for b in cell_data {
                            let b = b.reverse_bits();
                            bs.push_str(&format!("{b:08b}"));
                        }
                        let mut vote_index: Vec<Option<i32>> = bs
                            .match_indices("1")
                            .map(|(index, _)| Some(index as i32))
                            .collect();
                        vote_index.retain(|index| index.is_some());
                        if vote_index.len() == 0 {
                            error!(
                                "[vote]: tx({}) index({}) no valid vote index",
                                tx_hash.to_string(),
                                out_index
                            );
                            return Ok(());
                        }
                        let epoch_num = epoch_raw_data.epoch_number();
                        let epoch_index = epoch_raw_data.epoch_index();
                        let epoch_len = epoch_raw_data.epoch_length();
                        let epoch_raw = generate_epoch_raw(epoch_num, epoch_index, epoch_len)?;
                        let new_vote_record = models::NewVoteRecord {
                            address: ckb_addr.to_string(),
                            args,
                            height,
                            epoch_raw: epoch_raw as i64,
                            epoch_num: epoch_num as i64,
                            epoch_index: epoch_index as i64,
                            epoch_len: epoch_len as i64,
                            vote_index,
                            timestamp: transfer_time(timestamp),
                            tx_hash: tx_hash.to_string(),
                            tx_index,
                            out_index,
                        };
                        match insert_vote_record(conn, new_vote_record) {
                            Err(app_err) => {
                                error!(
                                    "[vote]: insert_vote_record failed: {}",
                                    app_err.to_string()
                                );
                            }
                            _ => {}
                        }
                    }
                    None => {
                        error!(
                            "[vote]: tx({}) index({}) out data not found",
                            tx_hash.to_string(),
                            out_index
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

fn parse_didoc_cell(cell_data: &[u8]) -> Result<Web5DocumentData, AppError> {
    let did_data =
        DidWeb5Data::from_slice(cell_data).map_err(|e| AppError::DagCborError(e.to_string()))?;
    let DidWeb5DataUnion::DidWeb5DataV1(did_data_v1) = did_data.to_enum();
    let did_doc: Bytes = did_data_v1.document();
    serde_ipld_dagcbor::from_slice(&did_doc.raw_data())
        .map_err(|e| AppError::DagCborError(e.to_string()))
}
