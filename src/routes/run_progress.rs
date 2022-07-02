use std::time::UNIX_EPOCH;
use uuid::Uuid;
use rocket::{serde::json::Json, http::Status};

use crate::{
    cache,
    run::{self, Summary, Tickstamp}, aws::{write_data_to_s3, push_summary_to_db},
};

fn start_time_key(run_id: &str) -> String {
    format!("{}-{}", "start_time", run_id)
}

#[get("/new-run")]
pub fn new_run<'a>() -> (Status, String) {
    let id = format!("{}", Uuid::new_v4());
    let start_time = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("bad time")
        .as_millis();
    let start_time = format!("{}", start_time);
    match cache::set(start_time_key(&id), start_time) {
        Ok(_) => (Status::Accepted, id),
        Err(e) => (
            Status::InternalServerError,
            format!("cache error: {}", e.msg),
        ),
    }
}

#[post("/run/<run_id>", data = "<post_data>")]
pub fn post_data<'a>(run_id: &str, post_data: &str) -> (Status, String) {
    let item_pairs: Vec<(&str, u64)> = post_data
        .split(',')
        .map(|t| {
            let score: u64 = t.trim().parse().expect("msg");
            (t.trim(), score)
        })
        .collect();

    match cache::zadd_multiple(run_id, item_pairs) {
        Ok(()) => (Status::Accepted, "".to_string()),
        Err(e) => (
            Status::InternalServerError,
            format!("cache error: {}", e.msg),
        ),
    }
}

#[derive(Responder)]
pub enum FinalizeRunResponse<'a> {
    #[response(status = 200)]
    Success(Json<Summary<'a>>),
    #[response(status = 500, content_type = "json")]
    Error(String),
}

#[post("/run/<run_id>/finish")]
pub async fn finalize_run<'a>(run_id: &'a str) -> FinalizeRunResponse<'a> {
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
