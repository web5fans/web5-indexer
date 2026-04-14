use crate::{
    crawl::CrawlManager,
    db::{
        dao::{insert_dao_record, query_valid_dao_record_by_output},
        did::{
            check_connection, delete_record, insert_record, query_valid_did_doc_by_index,
            query_valid_index_set,
        },
        pds::{insert_pds, query_pds, update_pds},
        vote::insert_vote_record,
    },
    error::{AppError, handle_error},
    models::{self, NewDaoRecord, PdsList},
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
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AppMode {
    DID,
    VOTE,
    DAO,
}

impl From<&str> for AppMode {
    fn from(value: &str) -> Self {
        match value {
            "did" | "DID" => Self::DID,
            "vote" | "VOTE" => Self::VOTE,
            "dao" | "DAO" => Self::DAO,
            _ => {
                panic!("The mode only can be did, vote or dao")
            }
        }
    }
}

#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum CellType {
    Did,
    Dao,
}

#[derive(Default)]
pub struct CkbCtx {
    valid_cells: HashSet<(H256, i32, CellType)>,
    pub token: CancellationToken,
    pub mode: Vec<AppMode>,
    pub crawl_manager: CrawlManager,
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
        crawl_manager: CrawlManager,
    ) -> Self {
        loop {
            match check_connection(conn) {
                Ok(_n) => {
                    break;
                }
                Err(_) => {
                    info!("Please create indexer schema");
                    time::sleep(Duration::from_secs(3)).await;
                }
            }
        }
        if mode.len() == 0 {
            panic!("App mode must be set, support \"vote\" & \"did\".");
        }
        let mut ctx = CkbCtx {
            valid_cells: HashSet::new(),
            token,
            mode,
            crawl_manager,
        };
        let live_cells = query_valid_index_set(conn).unwrap();
        info!("Ckb Ctx init. Found {} records", live_cells.len());
        for (str, idx) in live_cells {
            ctx.valid_cells
                .insert((H256::from_str(&str).unwrap(), idx, CellType::Did));
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
        dao_code_hash: &H256,
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
                    for (in_index, input) in tx.inner.inputs.iter().enumerate() {
                        if self.mode.contains(&AppMode::DID) {
                            self.did_input_handle(
                                conn,
                                in_index as i32,
                                input,
                                header.timestamp.value(),
                                tx.hash.to_string(),
                                query_height as i64,
                            )
                            .await?;
                        }
                        // if self.mode.contains(&AppMode::DAO) {
                        //     self.dao_input_handle(
                        //         conn,
                        //         in_index as i32,
                        //         input,
                        //         header.timestamp.value(),
                        //         tx.hash.to_string(),
                        //         query_height as i64,
                        //         tx_index as i32,
                        //     )?;
                        // }
                    }
                    for (out_inx, output) in tx.inner.outputs.iter().enumerate() {
                        if self.mode.contains(&AppMode::DID) {
                            self.did_output_handle(
                                conn,
                                &did_code_hash,
                                &tx.inner.outputs_data,
                                out_inx as i32,
                                output,
                                header.timestamp.value(),
                                tx.hash.clone(),
                                query_height as i64,
                                network,
                            )
                            .await?;
                        }
                        if self.mode.contains(&AppMode::VOTE) {
                            self.vote_output_handle(
                                conn,
                                &vote_code_hash,
                                &tx.inner.outputs_data,
                                out_inx as i32,
                                output,
                                header.epoch,
                                header.timestamp.value(),
                                tx_index as i32,
                                tx.hash.clone(),
                                query_height as i64,
                                network,
                            )?;
                        }
                        if self.mode.contains(&AppMode::DAO) {
                            self.dao_output_handle(
                                conn,
                                &dao_code_hash,
                                &tx.inner.outputs_data,
                                out_inx as i32,
                                output,
                                header.timestamp.value(),
                                tx_index as i32,
                                tx.hash.clone(),
                                query_height as i64,
                                network,
                                &tx.inner.inputs,
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

    pub async fn did_input_handle(
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
        if self
            .valid_cells
            .contains(&(pre_tx_hash.clone(), pre_index, CellType::Did))
        {
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
                &did_record.did,
                &did_record.handle,
                &did_record.signing_key,
                time_stamp,
                &did_record.ckb_address,
                &tx_hash,
                in_index,
                block_height,
                &did_record.document,
            ) {
                Err(app_err) => {
                    error!("[did]: delete_record failed: {}", app_err.to_string());
                    handle_error(app_err)?;
                }
                Ok(_) => {
                    if let Ok(didoc) =
                        serde_json::from_str::<Web5DocumentData>(&did_record.document)
                    {
                        if let Some(service) = didoc.services.get("atproto_pds") {
                            self.handle_pds(conn, &service.endpoint, false).await?;
                        } else {
                            error!(
                                "[did]: {}'s did doc not found atproto_pds service",
                                didoc.also_known_as[0]
                            );
                        }
                    } else {
                        error!(
                            "[did]: {}'s did doc not invalid: \n{}",
                            did_record.handle, did_record.document
                        )
                    }
                    self.valid_cells
                        .remove(&(pre_tx_hash, pre_index, CellType::Did));
                }
            };
        }
        Ok(())
    }

    pub async fn did_output_handle(
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
                    &didoc,
                    true,
                ) {
                    Err(app_err) => {
                        error!("[did]: insert_record failed: {}", app_err.to_string());
                        handle_error(app_err)?;
                    }
                    Ok(_) => {
                        if let Some(service) = didoc.services.get("atproto_pds") {
                            self.handle_pds(conn, &service.endpoint, true).await?;
                        } else {
                            error!(
                                "[did]: {}'s did doc not found atproto_pds service",
                                didoc.also_known_as[0]
                            );
                        }
                        self.valid_cells
                            .insert((tx_hash, out_inx as i32, CellType::Did));
                    }
                }
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
                                handle_error(app_err)?;
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

    #[allow(dead_code)]
    pub fn dao_input_handle(
        &mut self,
        conn: &mut PgConnection,
        in_index: i32,
        input: &CellInput,
        time_stamp: u64,
        tx_hash: String,
        block_height: i64,
        tx_index: i32,
    ) -> Result<(), AppError> {
        let pre_tx_hash = input.previous_output.tx_hash.clone();
        let pre_index = input.previous_output.index.value() as i32;
        if self
            .valid_cells
            .contains(&(pre_tx_hash.clone(), pre_index, CellType::Dao))
        {
            let dao_record =
                query_valid_dao_record_by_output(conn, &pre_tx_hash.to_string(), pre_index)?;
            match insert_dao_record(
                conn,
                NewDaoRecord {
                    ckb_address: dao_record.ckb_address,
                    tx_hash: tx_hash,
                    out_index: None,
                    in_index: Some(in_index),
                    ckb_number: dao_record.ckb_number,
                    deposit_or_withdraw: false,
                    height: block_height,
                    tx_index,
                    created_at: transfer_time(time_stamp),
                    valid: true,
                },
            ) {
                Err(app_err) => {
                    error!("[did]: delete_record failed: {}", app_err.to_string());
                    handle_error(app_err)?;
                }
                Ok(_) => {
                    self.valid_cells
                        .remove(&(pre_tx_hash, pre_index, CellType::Dao));
                }
            };
        }
        Ok(())
    }

    pub fn dao_output_handle(
        &mut self,
        conn: &mut PgConnection,
        dao_code_hash: &H256,
        outputs_data: &Vec<JsonBytes>,
        out_inx: i32,
        output: &CellOutput,
        time_stamp: u64,
        tx_index: i32,
        tx_hash: H256,
        block_height: i64,
        network: NetworkType,
        inputs: &[CellInput],
    ) -> Result<(), AppError> {
        if let Some(ref type_script) = output.type_ {
            if &type_script.code_hash == dao_code_hash {
                let ckb_addr = calculate_address(&output.lock.clone().into(), network);
                match outputs_data.get(out_inx as usize) {
                    Some(out_data) => {
                        let cell_data = out_data.as_bytes();
                        if cell_data != &[0; 8] {
                            if cell_data.len() == 8 {
                                let mut found_input = false;
                                for (in_index, input) in inputs.iter().enumerate() {
                                    let pre_tx_hash = input.previous_output.tx_hash.clone();
                                    let pre_index = input.previous_output.index.value() as i32;
                                    if let Ok(dao_record) = query_valid_dao_record_by_output(
                                        conn,
                                        &pre_tx_hash.to_string(),
                                        pre_index,
                                    ) {
                                        found_input = true;
                                        match insert_dao_record(
                                            conn,
                                            NewDaoRecord {
                                                ckb_address: dao_record.ckb_address,
                                                tx_hash: tx_hash.to_string(),
                                                out_index: None,
                                                in_index: Some(in_index as i32),
                                                ckb_number: dao_record.ckb_number,
                                                deposit_or_withdraw: false,
                                                height: block_height,
                                                tx_index,
                                                created_at: transfer_time(time_stamp),
                                                valid: true,
                                            },
                                        ) {
                                            Err(app_err) => {
                                                error!(
                                                    "[dao]: insert_dao_record failed: {}",
                                                    app_err.to_string()
                                                );
                                                handle_error(app_err)?;
                                            }
                                            Ok(_) => {
                                                // self.valid_cells.remove(&(
                                                //     pre_tx_hash,
                                                //     pre_index,
                                                //     CellType::Dao,
                                                // ));
                                            }
                                        }
                                    }
                                }
                                if !found_input {
                                    error!("[dao]: tx({}) data invalid", tx_hash.to_string(),);
                                }
                            } else {
                                error!(
                                    "[dao]: tx({}) out_index({}) not found any dao inputs",
                                    tx_hash.to_string(),
                                    out_inx
                                );
                            }
                        } else {
                            match insert_dao_record(
                                conn,
                                NewDaoRecord {
                                    ckb_address: ckb_addr.to_string(),
                                    tx_hash: tx_hash.to_string(),
                                    out_index: Some(out_inx),
                                    in_index: None,
                                    ckb_number: output.capacity.value() as i64,
                                    deposit_or_withdraw: true,
                                    height: block_height,
                                    tx_index,
                                    created_at: transfer_time(time_stamp),
                                    valid: true,
                                },
                            ) {
                                Err(app_err) => {
                                    error!(
                                        "[dao]: insert_dao_record failed: {}",
                                        app_err.to_string()
                                    );
                                    handle_error(app_err)?;
                                }
                                Ok(_) => {
                                    // self.valid_cells.insert((
                                    //     tx_hash,
                                    //     out_inx,
                                    //     CellType::Dao,
                                    // ));
                                }
                            }
                        }
                    }
                    None => {
                        error!(
                            "[dao]: tx({}) index({}) out data not found",
                            tx_hash.to_string(),
                            out_inx
                        );
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn handle_pds(
        &self,
        conn: &mut PgConnection,
        pds_url: &str,
        pon: bool, // positive or negative
    ) -> Result<(), AppError> {
        match query_pds(conn, pds_url) {
            Ok(mut pds) => {
                if pon {
                    pds.user_num += 1;
                } else {
                    pds.user_num -= 1;
                }
                match update_pds(conn, &pds) {
                    Err(app_err) => {
                        error!("[pds]: update_pds failed: {}", app_err.to_string());
                    }
                    _ => {}
                }
            }
            Err(AppError::DbRecordNotFound) => {
                if !pon {
                    error!("[pds]: can't be negative when pds not exist");
                } else if self.crawl_manager.check_pds(pds_url).await.is_ok() {
                    let new_pds = PdsList {
                        pds_url: pds_url.to_string(),
                        user_num: 1,
                    };
                    match insert_pds(conn, &new_pds) {
                        Err(app_err) => {
                            error!("[pds]: insert_pds failed: {}", app_err.to_string());
                            handle_error(app_err)?;
                        }
                        _ => {}
                    }
                    let url = Url::parse(pds_url).map_err(|e| {
                        AppError::PdsUrlError(format!("{pds_url}: {}", e.to_string()))
                    })?;
                    let scheme = url.scheme();
                    if scheme != "http" && scheme != "https" {
                        return Err(AppError::PdsUrlError(format!(
                            "{pds_url}: must be http or https"
                        )));
                    }
                    if let Some(host_url) = url.host_str() {
                        let host_url = if let Some(port) = url.port() {
                            format!("{host_url}:{port}")
                        } else {
                            format!("{host_url}")
                        };
                        let origin_list = self.crawl_manager.pds_list().await.unwrap_or_default();
                        if !origin_list.contains(&host_url) {
                            for _ in 0..3 {
                                match self.crawl_manager.wrap_crawl(&host_url).await {
                                    Ok(list) => {
                                        if list.contains(&host_url) {
                                            info!("[relay]: {host_url} already in pds list");
                                            break;
                                        }
                                        time::sleep(Duration::from_secs(3)).await;
                                        warn!("[relay]: {host_url} not in pds list");
                                    }
                                    Err(e) => {
                                        error!("[relay]: wrap_crawl error: {}", e.to_string());
                                        time::sleep(Duration::from_secs(3)).await;
                                    }
                                }
                            }
                        }
                    } else {
                        error!("[relay]: {pds_url} not a valid host");
                    }
                } else {
                    warn!("[relay]: {pds_url} not a valid web5 pds");
                }
            }
            Err(e) => return Err(e),
        }
        return Ok(());
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
