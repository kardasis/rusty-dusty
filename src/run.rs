use crate::constants::{
    self, INTERVAL_SIZE, MILLIS_PER_HOUR, SPEED_SMOOTHING, TICKS_PER_MILE, KILOMETERS_PER_MILE,
};
use json::{object, JsonValue};
use rocket::serde::Serialize;
use serde::Deserialize;
use std::collections::HashMap;

pub type Tickstamp = u32; // ms on device
type Timestamp = u32; // time within run in seconds, since start
type Speed = f32; // mph
type Distance = f32; // distance in miles

pub type DistanceRecordSet<'a> = HashMap<&'a str, Option<DistanceRecord>>;

#[derive(Debug, PartialEq)]
pub enum InvalidRunError {
    InsufficientData,
}

#[derive(Debug)]
struct SpeedPoint {
    time: Timestamp,
    speed: f32,
}
pub struct RawData {
    pub start_time: String,
    pub tickstamps: Vec<Tickstamp>,
}

impl RawData {
    pub fn generate_json(&self) -> JsonValue {
        object! {
            startTime: self.start_time.clone(),
            ticks: self.tickstamps.clone()
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct LargestRect {
    #[serde(rename = "start")]
    pub start_time: Timestamp,
    #[serde(rename = "end")]
    pub end_time: Timestamp,
    #[serde(rename = "height")]
    pub height: f32,
    #[serde(rename = "area")]
    pub area: f32,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct DistanceRecord {
    #[serde(rename = "left")]
    pub start_time: Timestamp,
    #[serde(rename = "right")]
    pub end_time: Timestamp,
    #[serde(rename = "leftD")]
    pub start_distance: Distance,
    #[serde(rename = "rightD")]
    pub end_distance: Distance,
    #[serde(rename = "time")]
    pub time: Timestamp,
}

#[derive(Debug, PartialEq)]
pub struct IntervalDatum {
    pub time: Timestamp,
    pub speed: Speed,
    pub calories: f32,
    pub distance: Distance,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Summary<'a> {
    #[serde(rename = "startTime")]
    pub start_time: String, // epoch time when run started
    #[serde(rename = "bestDistances")]
    pub distance_records: DistanceRecordSet<'a>,
    #[serde(rename = "totalTime")]
    pub total_time: u32,
    #[serde(rename = "maxRectangle")]
    pub largest_rect: LargestRect,
    #[serde(rename = "runId")]
    pub id: &'a str,
    #[serde(rename = "totalCalories")]
    pub total_calories: f32,
    #[serde(rename = "totalDistance")]
    pub total_distance: f32,
    #[serde(skip)]
    pub interval_data: Vec<IntervalDatum>,
}

impl Summary<'_> {
    pub fn new(id: &str, raw_data: RawData) -> Result<Summary, InvalidRunError> {
        let start_time = raw_data.start_time.clone();
        let id = id;
        let interval_data = Summary::calculate_interval_data(&raw_data, INTERVAL_SIZE);
        let total_time = Summary::calculate_total_time(&raw_data)?;

        let record_distances: HashMap<&'static str, f32> = HashMap::from([
            ("oneMile", 1.),
            ("fiveKm", 5. * KILOMETERS_PER_MILE),
            ("fiveMiles", 5.),
            ("fourMiles", 4.),
            ("halfMile", 0.5),
            ("lap", 0.25),
            ("oneKm", KILOMETERS_PER_MILE),
            ("tenKm", 10. * KILOMETERS_PER_MILE),
            ("threeMiles", 3.),
            ("twoMiles", 2.),
        ]);
        let distance_records =
            Summary::calculate_distance_records(&interval_data, record_distances);
        let largest_rect = Summary::calculate_largest_rect(&interval_data);
        let total_calories = Summary::calculate_total_calories(&interval_data);
        let total_distance = Summary::calculate_total_distance(&interval_data);
        Ok(Summary {
            start_time,
            total_time,
            distance_records,
            id,
            total_calories,
            largest_rect,
            total_distance,
            interval_data,
        })
    }

    fn calculate_interval_data(raw_data: &RawData, interval_length: u32) -> Vec<IntervalDatum> {
        let debounced_ticks = Summary::debounce(&raw_data);
        let mut res = vec![];
        let mut second: u32 = 1;
        let mut i: usize = 0;
        let mut speed = 0.0;

        let last_tick = match debounced_ticks.last() {
            Some(t) => t,
            None => return vec![],
        };

        while interval_length * second < *last_tick {
            let window_begin = i;
            while debounced_ticks
                .get(i)
                .expect("somehow i got out of range here")
                < &(interval_length * second)
            {
                i += 1;
            }
            let ticks_per_millis = ((i - window_begin) as f32)
                / ((debounced_ticks[i] - debounced_ticks[window_begin]) as f32);

            let immediate_speed = ticks_per_millis * MILLIS_PER_HOUR as f32 / TICKS_PER_MILE;
            speed = immediate_speed * (1. - SPEED_SMOOTHING) + SPEED_SMOOTHING * speed;
            let incline = 1.0;
            let weight = 192.0;
            let calories =
                (1.0 / 60 as f32) * (weight / 26400.) * (speed * (322. + 14.5 * incline) + 210.);
            res.push(IntervalDatum {
                time: second,
                speed,
                calories,
                distance: i as f32 / TICKS_PER_MILE,
            });
            second += 1;
        }
        res
    }

    fn debounce(raw_data: &RawData) -> Vec<Tickstamp> {
        let mut ticks = vec![];
        let mut prev_tick = 0; // value doesn't matter will be overwritten on first iteration
        let first_tick = match raw_data.tickstamps.first() {
            Some(t) => t,
            None => return ticks,
        };
        for (i, tick) in raw_data.tickstamps.iter().enumerate() {
            if i == 0 {
                prev_tick = 0;
            } else {
                let this_tick = tick - first_tick;
                if this_tick - prev_tick > constants::DEBOUNCE_TIME {
                    ticks.push(this_tick);
                    prev_tick = this_tick;
                }
            }
        }
        ticks
    }

    fn calculate_distance_record(
        data: &Vec<IntervalDatum>,
        distance: f32,
    ) -> Option<DistanceRecord> {
        let mut left: usize = 0;
        let mut right: usize = 0;
        let mut bests: Option<DistanceRecord> = None;

        'outer: while right < data.len() {
            while data[right].distance <= data[left].distance + distance {
                right += 1;
                if right >= data.len() {
                    break 'outer;
                }
            }
            bests = match bests {
                Some(ref b) => {
                    if data[right].time - data[left].time < b.time {
                        Some(DistanceRecord {
                            start_time: data[left].time,
                            end_time: data[right].time,
                            start_distance: data[left].distance,
                            end_distance: data[right].distance,
                            time: data[right].time - data[left].time,
                        })
                    } else {
                        bests
                    }

                    //  if (data[right].time - data[left].time) <  b.time {
                    //     mileTime = data[right].time - data[left].time
                    //     bestLeft = data[left].time
                    //     bestRight = data[right].time
                    //     leftD = data[left].distance
                    // rightD = data[right].distance
                }
                None => Some(DistanceRecord {
                    start_time: data[left].time,
                    end_time: data[right].time,
                    start_distance: data[left].distance,
                    end_distance: data[right].distance,
                    time: data[right].time - data[left].time,
                }),
            };
            left += 1
        }
        bests
    }

    fn calculate_distance_records<'a>(
        data: &Vec<IntervalDatum>,
        record_distances: HashMap<&'a str, f32>,
    ) -> DistanceRecordSet<'a> {
        let mut res = DistanceRecordSet::new();
        for (name, distance) in record_distances {
            res.insert(name, Summary::calculate_distance_record(data, distance));
        }
        res
    }

    fn calculate_total_time(raw_data: &RawData) -> Result<u32, InvalidRunError> {
        let first = raw_data
            .tickstamps
            .first()
            .ok_or(InvalidRunError::InsufficientData)?;
        let last = raw_data
            .tickstamps
            .last()
            .ok_or(InvalidRunError::InsufficientData)?;
        Ok((last - first)/1000)
    }

    fn calculate_total_calories(data: &Vec<IntervalDatum>) -> f32 {
        data.iter().map(|d| d.calories).sum()
    }

    fn calculate_total_distance(data: &Vec<IntervalDatum>) -> f32 {
        match data.last() {
            Some(d) => d.distance,
            None => 0.,
        }
    }

    fn calculate_largest_rect(data: &Vec<IntervalDatum>) -> LargestRect {
        let mut max_area_rect = LargestRect {
            start_time: data[0].time - 1,
            end_time: data[0].time,
            height: data[0].speed,
            area: data[0].speed,
        };
        let mut stack: Vec<SpeedPoint> = Vec::new();

        for (i, d) in data.iter().enumerate() {
            while stack.len() > 0 && (i == data.len() || stack.last().unwrap().speed >= d.speed) {
                let popped_bar = stack.pop().unwrap();
                let left_time = if stack.len() == 0 {
                    0
                } else {
                    stack.last().unwrap().time
                };
                let area = popped_bar.speed * (d.time - left_time) as f32;
                if area > max_area_rect.area {
                    max_area_rect = LargestRect {
                        start_time: left_time,
                        end_time: d.time,
                        height: popped_bar.speed,
                        area,
                    }
                }
            }
            stack.push(SpeedPoint {
                speed: d.speed,
                time: d.time,
            });
        }
        max_area_rect
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_largest_rect_success() {
        let data = vec![
            IntervalDatum {
                time: 1,
                speed: 1.0,
                calories: 0.,
                distance: 1.,
            },
            IntervalDatum {
                time: 2,
                speed: 2.0,
                calories: 0.,
                distance: 1.,
            },
            IntervalDatum {
                time: 3,
                speed: 3.0,
                calories: 0.,
                distance: 1.,
            },
            IntervalDatum {
                time: 4,
                speed: 4.0,
                calories: 0.,
                distance: 1.,
            },
            IntervalDatum {
                time: 5,
                speed: 5.0,
                calories: 0.,
                distance: 1.,
            },
            IntervalDatum {
                time: 5,
                speed: 0.0,
                calories: 0.,
                distance: 1.,
            },
        ];
        let lr = Summary::calculate_largest_rect(&data);
        assert_eq!(
            lr,
            LargestRect {
                start_time: 2,
                end_time: 5,
                height: 3.,
                area: 9.
            }
        )
    }
    #[test]
    fn calculate_total_time_fail() {
        let rd = RawData {
            start_time: "123456".to_string(),
            tickstamps: vec![],
        };
        let tt = Summary::calculate_total_time(&rd);
        assert_eq!(tt.unwrap_err(), InvalidRunError::InsufficientData);
    }

    #[test]
    fn calculate_total_time_success() {
        let rd = RawData {
            start_time: "123456".to_string(),
            tickstamps: vec![12123, 19456],
        };
        let tt = Summary::calculate_total_time(&rd);
        assert_eq!(tt.unwrap(), 7);
    }

    #[test]
    fn debouce_ticks_success() {
        let rd = RawData {
            start_time: "123456".to_string(),
            tickstamps: vec![6, 19, 40, 100],
        };
        let db = Summary::debounce(&rd);
        assert_eq!(db, vec![34, 94]);
    }

    #[test]
    fn calculate_interval_data_success() {
        let rd = RawData {
            start_time: "123456".to_string(),
            tickstamps: (1..100).map(|e| 40 * e).collect(),
        };
        let id = Summary::calculate_interval_data(&rd, 1000);
        assert_eq!(id.len(), 3);
        assert_eq!(
            id[0],
            IntervalDatum {
                time: 1,
                speed: 4.431818,
                calories: 0.20621902,
                distance: 0.0023636362
            }
        );
    }
}
