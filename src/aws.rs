use aws_sdk_dynamodb::{
    model::AttributeValue,
    model::AttributeValue::{M, N, S},
    Client as DynamoClient,
};
use aws_sdk_s3::{types::ByteStream as S3BytesStream, Client as S3Client};
use json::{stringify, JsonValue};
use std::{collections::HashMap, env, fs::File, io::Write, string::String};


use crate::run::{self, DistanceRecord, DistanceRecordSet, LargestRect, Summary};


pub async fn push_summary_to_db(summary: &Summary<'_>) -> Result<(), AwsError> {
    let shared_config = aws_config::load_from_env().await;
    let client = DynamoClient::new(&shared_config);
    let mut req = client
        .put_item()
        .table_name(env::var("AWS_DYNAMO_TABLE_SUMMARY").unwrap());
    for (k, v) in summary.attributes() {
        req = req.item(k, v);
    }
    match req.send().await {
        Ok(_) => Ok(()),
        Err(e) => Err(AwsError {
            msg: format!("error pushing summary to db: {}", e.to_string()),
        }),
    }
}

pub async fn write_data_to_s3(run_id: &str, data: JsonValue) -> Result<(), AwsError> {
    let shared_config = aws_config::load_from_env().await;
    let client = S3Client::new(&shared_config);

    let filepath = match write_file(run_id, data) {
        Ok(f) => f,
        Err(e) => {
            return Err(AwsError {
                msg: format!("unable to write file: {}", e.to_string()),
            });
        }
    };
    let bytestream = match S3BytesStream::read_from()
        .path(filepath.as_str())
        .build()
        .await
    {
        Ok(bs) => bs,
        Err(e) => {
            return Err(AwsError {
                msg: format!("error reading bytestream: {}", e.to_string()),
            })
        }
    };
    let req = client
        .put_object()
        .bucket(env::var("AWS_S3_RAW_DATA_BUCKET").unwrap())
        .body(bytestream)
        .key(run_id);

    match req.send().await {
        Ok(_) => match std::fs::remove_file(filepath) {
            Ok(_) => Ok(()),
            Err(e) => Err(AwsError {
                msg: format!("error deleting local file after upload: {}", e.to_string()),
            }),
        },
        Err(e) => Err(AwsError {
            msg: format!("error writing data to s3: {}", e.to_string()),
        }),
    }
}

#[derive(Debug)]
pub struct AwsError {
pub msg: String,
}

impl<'a> DistanceRecordSet<'a> {
fn to_hash_attribute(&self) -> HashMap<String, AttributeValue> {
    self.0
        .iter()
        .filter_map(|(k, v)| v.as_ref().map(|dr| (k.to_string(), dr.to_attribute())))
        .collect()
}
}

impl DistanceRecord {
fn to_attribute(&self) -> AttributeValue {
    let mut res = HashMap::new();
    res.insert("left".to_string(), N(self.start_time.to_string()));
    res.insert("leftD".to_string(), N(self.start_distance.to_string()));
    res.insert("right".to_string(), N(self.end_time.to_string()));
    res.insert("rightD".to_string(), N(self.end_distance.to_string()));
    res.insert("time".to_string(), N(self.time.to_string()));
    M(res)
}
}

impl LargestRect {
fn to_hash_attribute(&self) -> HashMap<String, AttributeValue> {
    HashMap::from([
        ("start".to_string(), N(self.start_time.to_string())),
        ("end".to_string(), N(self.end_time.to_string())),
        ("height".to_string(), N(self.height.to_string())),
        ("area".to_string(), N(self.area.to_string())),
    ])
}
}

impl<'a> Summary<'a> {
fn attributes(&self) -> HashMap<&str, AttributeValue> {
    HashMap::from([
        ("runId", S(self.id.to_string())),
        ("totalTime", N(self.total_time.to_string())),
        ("startTime", N(self.start_time.clone())),
        ("totalCalories", N(self.total_calories.to_string())),
        ("totalDistance", N(self.total_distance.to_string())),
        ("maxRectangle", M(self.largest_rect.to_hash_attribute())),
        (
            "bestDistances",
            M(self.distance_records.to_hash_attribute()),
        ),
    ])
}
}

impl From<run::DistanceRecord> for HashMap<&str, AttributeValue> {
    fn from(item: run::DistanceRecord) -> Self {
        let time = item.end_time - item.start_time;
        HashMap::from([
            ("left", N(item.start_time.to_string())),
            ("right", N(item.end_time.to_string())),
            ("leftD", N(item.start_distance.to_string())),
            ("rightD", N(item.end_distance.to_string())),
            ("time", N(time.to_string())),
        ])
    }
}

fn write_file<T>(run_id: &str, data: T) -> Result<String, std::io::Error>
where
    T: Into<JsonValue>,
{
    let path = data_file_path(run_id);
    let mut file = File::create(&path)?;
    file.write_all(stringify(data).as_bytes())?;
    Ok(path)
}

fn data_file_path(run_id: &str) -> String {
    format!("./tmp/{}.json", run_id)
}
