use std::{env, num::ParseIntError, str::FromStr, thread::sleep};

use chrono::{FixedOffset, NaiveDateTime, TimeZone, Utc};
use cron::Schedule;
use env_logger::{init_from_env, Env};
use log::{error, info};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct BookingsDto {
    #[serde(rename = "groupActivities")]
    group_activities: Vec<GroupActivity>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GroupActivity {
    #[serde(rename = "Id")]
    id: i64,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "ImageUrl")]
    image_url: Option<serde_json::Value>,
    #[serde(rename = "Description")]
    description: Option<serde_json::Value>,
    #[serde(rename = "Message")]
    message: Option<String>,
    #[serde(rename = "Status")]
    status: Status,
    #[serde(rename = "StartTime")]
    start_time: String,
    #[serde(rename = "EndTime")]
    end_time: String,
    #[serde(rename = "Location")]
    location: Location,
    #[serde(rename = "Instructor")]
    instructor: String,
    #[serde(rename = "InstructorId")]
    instructor_id: i64,
    #[serde(rename = "FreeSlots")]
    free_slots: i64,
    #[serde(rename = "Dropin")]
    dropin: i64,
    #[serde(rename = "DropsAmount")]
    drops_amount: i64,
    #[serde(rename = "BookingId")]
    booking_id: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Location {
    #[serde(rename = "Gym Torsvik")]
    GymTorsvik,
    #[serde(rename = "Sal A Torsvik")]
    SalATorsvik,
    #[serde(rename = "Sal B Torsvik")]
    SalBTorsvik,
    #[serde(rename = "Sal C Torsvik")]
    SalCTorsvik,
    #[serde(rename = "Sal Easy Line Torsvik")]
    SalEasyLineTorsvik,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Status {
    Bookable,
    Unavailable,
    Booked,
}

fn get_nw_date(time: &NaiveDateTime) -> String {
    // create a timezone instance of UTC+2 = Sweden
    let swe_tz = FixedOffset::east_opt(2 * 3600).expect("Time out of bounds");
    swe_tz
        .from_utc_datetime(time)
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

fn get_bookings_url(user_id: &str) -> String {
    let now = Utc::now().naive_utc();
    let today = get_nw_date(&now);
    let in_one_week = get_nw_date(&(now + chrono::Duration::weeks(1)));
    info!("Fetching activities between {today} and {in_one_week}");
    format!("https://api1.nordicwellness.se/GroupActivity/timeslot?clubIds=1&activities=&dates={today}%2C{in_one_week}&time=&employees=&times=09%3A00-11%3A00%2C17%3A00-22%3A00&datespan=true&userId={user_id}")
}

fn book_activity(
    activity_id: u32,
    user_id: u32,
) -> Result<reqwest::blocking::Response, reqwest::Error> {
    let url = "https://api1.nordicwellness.se/Booking";
    let client = reqwest::blocking::Client::new();
    let body_data = (
        ("ActivityId", activity_id),
        ("UserId", user_id),
        ("QueueType", "ordinary"),
    );

    let body = reqwest::blocking::Body::from(serde_urlencoded::to_string(body_data).unwrap());

    client
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
}

fn book_body_balance(user_id: u32) {
    let response = reqwest::blocking::get(get_bookings_url(&user_id.to_string())).unwrap();
    let dto: BookingsDto = serde_json::from_str(&response.text().unwrap()).unwrap();
    let body_balance_activity = dto
        .group_activities
        .iter()
        .find(|it| it.name == "BODYBALANCE® 60");
    let activity = match body_balance_activity {
        Some(it) => it,
        None => {
            error!("Unable to find activity with the correct name");
            error!("Available activities: {:?}", dto);
            return;
        }
    };
    info!(
        "Found {} starting at time {}. Attempting to book it!",
        activity.name, activity.start_time
    );

    let response = book_activity(activity.id as u32, user_id);
    match response {
        Ok(res) => match res.status() {
            StatusCode::OK => info!("Booked {}", activity.name),
            StatusCode::BAD_REQUEST => info!("BAD_REQUEST: {}", res.text().unwrap()),
            code => error!("Encountered status code {code} which is not handled!"),
        },
        Err(e) => error!("Booking failed with error {}", e),
    }
}

struct UserIds(Vec<u32>);

impl FromStr for UserIds {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ids = s
            .split(",")
            .filter_map(|it| it.parse::<u32>().ok())
            .collect::<Vec<_>>();
        Ok(Self { 0: ids })
    }
}

fn main() {
    init_from_env(Env::new().default_filter_or("info"));

    let every_sunday_at_1600 = "0 15 12 * * Sun *";
    // create a timezone instance of UTC+2 = Sweden
    let swe_tz = FixedOffset::east_opt(2 * 3600).expect("Time out of bounds");
    let schedule = Schedule::from_str(every_sunday_at_1600).expect(&format!(
        "Unable to parse cron expression {every_sunday_at_1600}"
    ));
    let user_ids = env::var("USER_IDS").unwrap().parse::<UserIds>().unwrap();

    for next_time in schedule.upcoming(swe_tz) {
        let now = swe_tz.from_utc_datetime(&Utc::now().naive_utc());
        let wait_time = next_time - now;
        let sleep_sec = core::time::Duration::from_secs(wait_time.num_seconds() as u64);
        sleep(sleep_sec);
        for id in &user_ids.0 {
            book_body_balance(*id);
        }
    }
}
