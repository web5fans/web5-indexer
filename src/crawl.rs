use crate::error::AppError;
use reqwest::Client;
use serde::Serialize;
use serde_json::json;

#[derive(Serialize)]
struct CrawlParam {
    hostname: String,
}

#[derive(Serialize)]
struct LimitsParam {
    per_second: i64,
    per_hour: i64,
    per_day: i64,
    crawl_rate: i64,
    repo_limit: i64,
    host: String,
}

impl Default for LimitsParam {
    fn default() -> Self {
        Self {
            per_second: 100,
            per_hour: 1000000,
            per_day: 1000000,
            crawl_rate: 10,
            repo_limit: 1000000,
            host: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CrawlManager {
    pub relay_url: String,
    pub bearer_auth: String,
    pub auto_relay: bool,
}

impl CrawlManager {
    pub fn new(relay_url_opt: Option<String>, bearer_auth_opt: Option<String>) -> Self {
        if relay_url_opt.is_some() && bearer_auth_opt.is_some() {
            Self {
                relay_url: relay_url_opt.unwrap(),
                bearer_auth: bearer_auth_opt.unwrap(),
                auto_relay: true,
            }
        } else {
            Default::default()
        }
    }

    pub async fn check_pds(&self, pds_url: &str) -> Result<(), AppError> {
        let client = Client::new();
        let res = client
            .post(format!("{pds_url}/xrpc/fans.web5.ckb.indexQuery"))
            .json(&json!({
                "index": {
                    "$type": "fans.web5.ckb.indexQuery#firstItem",
                },
            }))
            .send()
            .await
            .map_err(|e| AppError::PdsUrlError(e.to_string()))?;
        if !res.status().is_success() {
            return Err(AppError::PdsUrlError(format!("{res:?}")));
        }
        Ok(())
    }

    pub async fn request_crawl(&self, pds_url: &str) -> Result<(), AppError> {
        if self.auto_relay {
            let client = Client::new();
            let param = CrawlParam {
                hostname: pds_url.to_string(),
            };

            let res = client
                .post(format!("{}/admin/pds/requestCrawl", self.relay_url))
                .bearer_auth(&self.bearer_auth)
                .json(&param)
                .send()
                .await
                .map_err(|e| AppError::RelayHttpError(e.to_string()))?;
            info!("[crawl]: request_crawl response: {:?}", res);
            if !res.status().is_success() {
                return Err(AppError::RelayHttpError(format!("{res:?}")));
            }
        }
        Ok(())
    }

    pub async fn change_limits(&self, pds_url: &str) -> Result<(), AppError> {
        if self.auto_relay {
            let client = Client::new();
            let param = LimitsParam {
                host: pds_url.to_string(),
                ..Default::default()
            };

            let res = client
                .post(format!("{}/admin/pds/changeLimits", self.relay_url))
                .bearer_auth(&self.bearer_auth)
                .json(&param)
                .send()
                .await
                .map_err(|e| AppError::RelayHttpError(e.to_string()))?;
            info!("[crawl]: change_limits response: {:?}", res);
            if !res.status().is_success() {
                return Err(AppError::RelayHttpError(format!("{res:?}")));
            }
        }
        Ok(())
    }

    pub async fn pds_list(&self) -> Result<String, AppError> {
        if self.auto_relay {
            let client = Client::new();

            let list = client
                .get(format!("{}/admin/pds/list", self.relay_url))
                .bearer_auth(&self.bearer_auth)
                .send()
                .await
                .map_err(|e| AppError::RelayHttpError(e.to_string()))?
                .text()
                .await
                .map_err(|e| {
                    AppError::RelayHttpError(format!(
                        "[crawl]: get pds list response error: {}",
                        e.to_string()
                    ))
                })?;
            info!("[crawl]: pds_list response: {list}");
            Ok(list)
        } else {
            Err(AppError::RunTimeError(format!(
                "[crawl]: not auto relay mode"
            )))
        }
    }

    pub async fn wrap_crawl(&self, pds_url: &str) -> Result<String, AppError> {
        self.request_crawl(pds_url).await?;
        self.change_limits(pds_url).await?;
        self.pds_list().await
    }
}
