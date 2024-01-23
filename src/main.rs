use chrono::{DateTime, Datelike, FixedOffset, NaiveDateTime, TimeZone, Utc, Weekday};
use env_logger::{init_from_env, Env};
use eyre::{Error, Result};
use log::{error, info};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;

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
    status: String,
    #[serde(rename = "StartTime")]
    start_time: String,
    #[serde(rename = "EndTime")]
    end_time: String,
    #[serde(rename = "Location")]
    location: String,
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

fn get_nw_date(time: &NaiveDateTime) -> String {
    // create a timezone instance of UTC+2 = Sweden
    let swe_tz = FixedOffset::east_opt(2 * 3600).expect("Time out of bounds");
    swe_tz
        .from_utc_datetime(time)
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

fn get_bookings_url(user_id: &str, activity_id: &str) -> String {
    let now = Utc::now().naive_utc();
    let today = get_nw_date(&now);
    let in_one_week = get_nw_date(&(now + chrono::Duration::weeks(1)));
    format!("https://api1.nordicwellness.se/GroupActivity/timeslot?clubIds=1&activities={activity_id}&dates={today}%2C{in_one_week}&time=&employees=&times=09%3A00-11%3A00%2C17%3A00-22%3A00&datespan=true&userId={user_id}")
}

async fn book_activity(
    activity_id: u32,
    user_id: u32,
) -> Result<reqwest::Response, reqwest::Error> {
    let url = "https://api1.nordicwellness.se/Booking";
    let client = reqwest::Client::new();
    let body_data = (
        ("ActivityId", activity_id),
        ("UserId", user_id),
        ("QueueType", "ordinary"),
    );

    let body = reqwest::Body::from(serde_urlencoded::to_string(body_data).unwrap());

    client
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
}

fn parse_date(date_str: &str) -> DateTime<Utc> {
    // This is dates in a swedish tz
    let date_str = date_str.to_string() + "+02:00";
    let datetime = DateTime::parse_from_rfc3339(&date_str).unwrap();
    datetime.with_timezone(&Utc)
}

async fn attempt_to_book_activity(activity: BookableActivity) -> Result<()> {
    let url = get_bookings_url(&activity.user_id.to_string(), &activity.id);
    info!(
        "sending request to get activities with id {} for user {}",
        &activity.id, &activity.user_name
    );
    info!("{}", url);
    let response = reqwest::get(url).await?;

    let dto: BookingsDto = serde_json::from_str(&response.text().await?)?;
    let body_balance_activity = dto.group_activities.iter().find(|it| {
        let is_same_name = it
            .name
            .to_lowercase()
            .contains(&activity.name.to_lowercase());
        let is_correct_day = parse_date(&it.start_time).weekday()
            == parse_weekday(&activity.day).expect("invalid week day");
        let is_correct_status = it.status == "Bookable";
        is_same_name && is_correct_day && is_correct_status
    });
    let nw_activity = match body_balance_activity {
        Some(it) => it,
        None => {
            info!(
                "Unable to find activity with name {} day {} and status {}",
                &activity.name, &activity.day, "Bookable"
            );
            let json = serde_json::to_string_pretty(&dto).unwrap();
            info!("{}", json);
            return Ok(());
        }
    };
    info!(
        "Found {} starting at time {}. Attempting to book it",
        nw_activity.name, nw_activity.start_time
    );

    let response = book_activity(nw_activity.id as u32, activity.user_id).await?;
    let status = response.status();
    let text = response.text().await?;
    if status != StatusCode::OK {
        let err_msg = format!("code {}: {}", status.as_str(), text,);
        return Err(Error::msg(err_msg));
    }
    info!("{}", text);
    info!("Booked {}", nw_activity.name);
    Ok(())
}

fn read_json<T: DeserializeOwned>(path: &str) -> T {
    let mut file = File::open(path).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("unable to read file");

    serde_json::from_str::<T>(&contents).expect("unable to deserialize json")
}

#[derive(Deserialize, Clone, Debug)]
struct BookableActivity {
    name: String,
    id: String,
    user_id: u32,
    day: String,
    user_name: String,
    disabled: Option<bool>,
}

fn parse_weekday(value: &str) -> Option<Weekday> {
    match value.to_lowercase().as_str() {
        "sun" | "sunday" => Some(Weekday::Sun),
        "mon" | "monday" => Some(Weekday::Mon),
        "tue" | "tuesday" => Some(Weekday::Tue),
        "wed" | "wednesday" => Some(Weekday::Wed),
        "thu" | "thursday" => Some(Weekday::Thu),
        "fri" | "friday" => Some(Weekday::Fri),
        "sat" | "saturday" => Some(Weekday::Sat),
        _ => None,
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    init_from_env(Env::new().default_filter_or("info"));
    let bookable_activities: Vec<BookableActivity> = read_json("./assets/bookable-activities.json");
    let all_bookable_activities = bookable_activities.len();
    info!("found {} bookable activities", all_bookable_activities);
    let mut handles = vec![];
    let bookable_activities: Vec<BookableActivity> = bookable_activities
        .into_iter()
        .filter(|it| !it.disabled.unwrap_or(false))
        .collect();
    info!(
        "removed {} disabled activities",
        all_bookable_activities - bookable_activities.len()
    );
    info!("trying to book {} activities", bookable_activities.len());

    for activity in bookable_activities {
        info!(
            "checking activity {} for user {}",
            &activity.name, &activity.user_name
        );
        let handle = tokio::task::spawn(async move {
            match attempt_to_book_activity(activity).await {
                Ok(()) => (),
                Err(err) => error!("{}", err.to_string()),
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        match handle.await {
            Ok(_) => (),
            Err(e) => error!("{}", e.to_string()),
        };
    }
    Ok(())
}
