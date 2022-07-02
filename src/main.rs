#[cfg(test)] mod integration_tests;

mod aws;
mod cache;
mod constants;
mod routes;
mod run;

use dotenv::dotenv;

#[macro_use]
extern crate rocket;

#[launch]
fn launch() -> _ {
    dotenv().ok();
    rocket::build()
    .mount(
        "/",
        routes![
            routes::post_data,
            routes::new_run,
            routes::finalize_run
        ],
    )
}