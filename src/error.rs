use actix_web::{HttpResponse, ResponseError};
use derive_more::Display;
use serde::Serialize;
use std::io;

#[derive(Clone, Debug, Display, PartialEq)]
pub enum AppError {
    #[display("Db record not found")]
    DbRecordNotFound,
    #[display("Db insert or update operation failed")]
    DbInsOrUpFailed,
    #[display("Did not registered: {_0}")]
    DidDocNotFound(String),
    #[display("Count not found")]
    CountNotFound,
    #[display("Did not available: {_0}")]
    DidDocNoData(String),
    #[display("Db execution failed: {_0}")]
    DbExecuteFailed(String),
    #[display("Runtime Internal Error: {_0}")]
    RunTimeError(String),
    #[display("Molecule decode Error: {_0}")]
    MoleculeError(String),
    #[display("Dag-cbor decode Error: {_0}")]
    DagCborError(String),
    #[display("Did document in compatible: {_0}")]
    IncompatibleDidDoc(String),
    #[display("Did format in compatible: {_0}")]
    IncompatibleDid(String),
    #[display("Db record count in compatible: {_0}")]
    DbCountError(String),
    #[display("Ckb node rpc error: {_0}")]
    CkbRpcError(String),
    #[display("Ckb address not registered: {_0}")]
    CkbAddrNotFound(String),
    #[display("Vote parameters error: {_0}")]
    VoteParamsError(String),
    #[display("Pds url valid: {_0}")]
    PdsUrlError(String),
    #[display("Relay Http request error: {_0}")]
    RelayHttpError(String),
    #[display("Dao stake' sum is negative")]
    DaoStakeNegError,
    #[display("Dao stake's query list size must <= 20")]
    DaoStakeOverLimitError,
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        let (status_code, error_msg) = match self {
            AppError::DbRecordNotFound => (404, self.to_string()),
            AppError::DbInsOrUpFailed => (404, self.to_string()),
            AppError::DidDocNotFound(_) => (404, self.to_string()),
            AppError::DidDocNoData(_) => (404, self.to_string()),
            AppError::DbExecuteFailed(_) => (500, self.to_string()),
            AppError::RunTimeError(_) => (500, self.to_string()),
            AppError::MoleculeError(_) => (500, self.to_string()),
            AppError::DagCborError(_) => (500, self.to_string()),
            AppError::IncompatibleDidDoc(_) => (500, self.to_string()),
            AppError::CountNotFound => (500, self.to_string()),
            AppError::DbCountError(_) => (500, self.to_string()),
            AppError::CkbRpcError(_) => (500, self.to_string()),
            AppError::IncompatibleDid(_) => (500, self.to_string()),
            AppError::CkbAddrNotFound(_) => (404, self.to_string()),
            AppError::VoteParamsError(_) => (500, self.to_string()),
            AppError::RelayHttpError(_) => (500, self.to_string()),
            AppError::PdsUrlError(_) => (500, self.to_string()),
            AppError::DaoStakeNegError => (500, self.to_string()),
            AppError::DaoStakeOverLimitError => (500, self.to_string()),
        };
        let error_response = ErrorResponse { message: error_msg };

        HttpResponse::build(actix_web::http::StatusCode::from_u16(status_code).unwrap())
            .json(error_response)
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    message: String,
}

impl From<io::Error> for AppError {
    fn from(value: io::Error) -> Self {
        AppError::RunTimeError(value.to_string())
    }
}

pub fn handle_error(e: AppError) -> Result<(), AppError> {
    match e {
        AppError::DbRecordNotFound | AppError::DbInsOrUpFailed => Ok(()),
        e => Err(e),
    }
}
