use crate::{
    cell_data::{DidWeb5Data, DidWeb5DataUnion},
    error::AppError,
    types::Web5DocumentData,
};
use chrono::offset::Utc as UtcOffset;
use chrono::{DateTime, Duration};
use ckb_sdk::{Address, AddressPayload, NetworkType};
use ckb_types::packed::Script;
use data_encoding::BASE32;
use molecule::prelude::Entity;
use std::time::SystemTime;

pub const RFC3339_F: &str = "%Y-%m-%dT%H:%M:%S%.3fZ";

pub fn parse_molecule(bytes: &[u8]) -> Result<Web5DocumentData, AppError> {
    let did_data = DidWeb5Data::from_slice(bytes).map_err(|_| {
        AppError::MoleculeError("DidWeb5Data convert failed, please update cell.".to_string())
    })?;
    let DidWeb5DataUnion::DidWeb5DataV1(did_data_v1) = did_data.to_enum();
    let did_doc = did_data_v1.document();
    Ok(
        serde_ipld_dagcbor::from_slice(&did_doc.raw_data()).map_err(|e| {
            AppError::DagCborError(format!(
                "Web5DocumentData dog cbor decode failed: {e:?}, please update cell."
            ))
        })?,
    )
}

pub fn check_did_doc(doc: &Web5DocumentData) -> Result<(String, String), AppError> {
    if doc.also_known_as.len() == 0 || !doc.also_known_as[0].starts_with("at://") {
        return Err(AppError::IncompatibleDidDoc(format!(
            "alsoKnownAs not correct: {:?}",
            doc.also_known_as
        )));
    }
    if doc.services.len() == 0 {
        return Err(AppError::IncompatibleDidDoc(format!(
            "services not provide",
        )));
    }
    let handle = doc.also_known_as[0][5..].to_string();
    if let Some(key) = doc.verification_methods.get("atproto") {
        if !check_signing_key_str(key) {
            Err(AppError::IncompatibleDidDoc(format!(
                "verificationMethods provided signing key format error: {key}",
            )))
        } else {
            Ok((handle, key.clone()))
        }
    } else {
        Err(AppError::IncompatibleDidDoc(format!(
            "verificationMethods not provide",
        )))
    }
}

pub fn check_did_str(_did: &str) -> bool {
    // did.starts_with("did:web5")
    true
}

pub fn extract_core_did(did: &str) -> String {
    if did.starts_with("did:") {
        did.split(':').last().unwrap().to_string()
    } else {
        did.to_string()
    }
}

pub fn check_signing_key_str(did: &str) -> bool {
    did.starts_with("did:key")
}

pub fn transfer_time(ts: u64) -> String {
    let unix_time = SystemTime::UNIX_EPOCH;
    let mut dt: DateTime<UtcOffset> = unix_time.into();
    dt = dt + Duration::milliseconds(ts as i64);
    format!("{}", dt.format(RFC3339_F))
}

pub fn calculate_web5_did(args: &[u8]) -> String {
    // format!("did:web5:{}", BASE32.encode(args).to_lowercase())
    BASE32.encode(args).to_lowercase()
}

pub fn calculate_address(lock_script: &Script, network: NetworkType) -> Address {
    let payload = AddressPayload::from(lock_script.clone());
    Address::new(network, payload, true)
}
