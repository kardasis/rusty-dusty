use rocket::serde::json::Json;

use crate::{
    aws::{push_summary_to_db, write_data_to_s3},
    cache,
    run::{self, Summary, Tickstamp},
    start_time_key,
};

#[derive(Responder)]
pub enum FinalizeRunResponse<'a> {
    #[response(status = 200)]
    Success(Json<Summary<'a>>),
    #[response(status = 500, content_type = "json")]
    Error(String),
}

#[post("/run/<run_id>/finish")]
pub async fn finialize_run<'a>(run_id: &'a str) -> FinalizeRunResponse<'a> {
    let tickstamp_data = match cache::fullzrange(run_id) {
        Ok(d) => d,
        Err(e) => {
            return FinalizeRunResponse::Error(format!(
                "error fetching tickstamps from cache: {}",
                e.msg
            ))
        }
    };
    let start_time = match cache::get(start_time_key(run_id)) {
        Ok(t) => t,
        Err(e) => {
            return FinalizeRunResponse::Error(format!(
                "error fetching start time from cache: {}",
                e.msg
            ))
        }
    };
    let mut tickstamps: Vec<Tickstamp> = Vec::new();
    for t in &tickstamp_data {
        let val = match t.parse() {
            Ok(t) => t,
            Err(_) => continue,
        };
        tickstamps.push(val);
    }

    let raw_data = run::RawData {
        tickstamps,
        start_time,
    };

    let data = raw_data.generate_json();

    match write_data_to_s3(run_id, data).await {
        Ok(_) => match cache::zrem(run_id) {
            Ok(_) => match cache::del(&start_time_key(run_id)) {
                Ok(_) => (),
                Err(e) => {
                    return FinalizeRunResponse::Error(format!(
                        "failed to remove start key from cache: {}",
                        e.msg
                    ))
                }
            },
            Err(e) => {
                return FinalizeRunResponse::Error(format!(
                    "failed to remove tickstamp data from cache: {}",
                    e.msg
                ))
            }
        },
        Err(e) => {
            return FinalizeRunResponse::Error(format!("failed to write data to s3: {}", e.msg))
        }
    }

    match Summary::new(run_id, raw_data) {
        Ok(summary) => match push_summary_to_db(&summary).await {
            Ok(()) => FinalizeRunResponse::Success(Json(summary)),
            Err(e) => {
                FinalizeRunResponse::Error(format!("failed to push summary to db: {}", e.msg))
            }
        },
        Err(_) => FinalizeRunResponse::Error("failed to create summary of run".to_string()),
    }
}
