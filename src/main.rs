#[cfg(test)] mod integration_tests;

mod aws;
mod cache;
mod constants;
mod routes;
mod run;

use dotenv::dotenv;
use rocket::http::Status;

use std::time::UNIX_EPOCH;
use uuid::Uuid;

#[macro_use]
extern crate rocket;
extern crate redis;

#[get("/")]
fn index() -> String {
    format!("hello, I'm rusty-dusty")
}

#[get("/new-run")]
fn new_run<'a>() -> (Status, String) {
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
fn post_data<'a>(run_id: &str, post_data: &str) -> (Status, String) {
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

fn start_time_key(run_id: &str) -> String {
    format!("{}-{}", "start_time", run_id)
}

#[launch]
fn rocket() -> _ {
    dotenv().ok();
    rocket::build().mount(
        "/",
        routes![
            post_data,
            new_run,
            index,
            routes::finalize_run::finialize_run
        ],
    )
}
