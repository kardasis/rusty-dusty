
use crate::run::Summary;

#[cfg(test)]
use super::*;
use regex::Regex;
use rocket::{local::blocking::Client, serde::json};
use dotenv;

#[ignore]
#[test]
fn push_data_and_finalize() {
    dotenv().ok();
    let rocket = rocket::build().mount(
        "/",
        routes![routes::post_data, routes::new_run, routes::finalize_run],
    );
    let client = Client::tracked(rocket).expect("valid rocket instance");

    let response = client.get("/new-run").dispatch();

    let re = Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").unwrap();
    let run_id = response.into_string().unwrap();
    assert!(re.is_match(run_id.as_ref()));

    let req_count = 5000;
    let ticks_per_req = 10;
    let ms_per_tick = 30;
    let ms_per_req = ticks_per_req * ms_per_tick;
    (0..req_count).for_each(|r| {
        let ms: Vec<String> = (0..ticks_per_req)
            .map(|t| (ms_per_req * r + ms_per_tick * t).to_string())
            .collect();
        let data = ms.join(",");
        client
            .post(format!("/run/{}", run_id))
            .body(data)
            .dispatch();
    });
    assert_eq!(
        cache::fullzrange(&run_id).unwrap().len(),
        req_count * ticks_per_req
    );

    let response = client.post(format!("/run/{}/finish", run_id)).dispatch();
    let summary_response = response.into_string().unwrap();
    let actual_summary: Summary = json::from_str(&summary_response).unwrap();
    let expected_summary_str = "{\"startTime\":\"1656202584971\",\"bestDistances\":{\"fiveMiles\":null,\"halfMile\":{\"left\":1,\"right\":154,\"leftD\":0.0032499998,\"rightD\":0.5055227,\"time\":153},\"twoMiles\":null,\"fiveKm\":null,\"oneMile\":null,\"lap\":{\"left\":1,\"right\":78,\"leftD\":0.0032499998,\"rightD\":0.2559621,\"time\":77},\"tenKm\":null,\"threeMiles\":null,\"oneKm\":null,\"fourMiles\":null},\"totalTime\":299,\"maxRectangle\":{\"start\":24,\"end\":299,\"height\":11.818182,\"area\":3250.0},\"runId\":\"10ef491c-426c-406c-a885-15fbf1e0e9e0\",\"totalCalories\":151.2582,\"totalDistance\":0.98149997}";

    let expected_summary: Summary = json::from_str(expected_summary_str).unwrap();
    assert_eq!(expected_summary, actual_summary);

    assert_eq!(cache::fullzrange(&run_id).unwrap().len(), 0);

    // Teardown
    cache::flushdb().expect("problem flushing cache");
}
