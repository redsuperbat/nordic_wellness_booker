use std::{env, str::FromStr};

use async_recursion::async_recursion;
use chrono::{Duration, FixedOffset, Local, NaiveDateTime, TimeZone, Utc};
use cron::Schedule;
use cron_descriptor::cronparser::cron_expression_descriptor::get_description_cron;
use env_logger::{init_from_env, Env};
use eyre::{Error, Result};
use humantime::format_duration;
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

fn get_bookings_url(user_id: &str) -> String {
    let now = Utc::now().naive_utc();
    let in_six_days = get_nw_date(&(now + Duration::days(6)));
    let in_one_week = get_nw_date(&(now + chrono::Duration::weeks(1)));
    info!("Fetching activities between {in_six_days} and {in_one_week}");
    format!("https://api1.nordicwellness.se/GroupActivity/timeslot?clubIds=1&activities=&dates={in_six_days}%2C{in_one_week}&time=&employees=&times=09%3A00-11%3A00%2C17%3A00-22%3A00&datespan=true&userId={user_id}")
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

async fn find_activity_by_name(activity: BookableActivity) -> Result<()> {
    let response = reqwest::get(get_bookings_url(&activity.user_id.to_string())).await?;
    let dto: BookingsDto = serde_json::from_str(&response.text().await?)?;
    let body_balance_activity = dto.group_activities.iter().find(|it| {
        it.name == activity.name
            && it.start_time.ends_with(&activity.start_time)
            && it.status == "Bookable"
    });
    let nw_activity = match body_balance_activity {
        Some(it) => it,
        None => {
            error!("Unable to find activity with the correct name");
            let json = serde_json::to_string_pretty(&dto).unwrap();
            error!("{}", json);
            return Ok(());
        }
    };
    info!(
        "Found {} starting at time {}. Attempting to book it!",
        nw_activity.name, nw_activity.start_time
    );

    let response = book_activity(nw_activity.id as u32, activity.user_id).await?;
    let status = response.status();
    let text = response.text().await?;
    info!("Status Code {}", status.as_str());
    info!("{}", text);
    if StatusCode::OK != status {
        return Err(Error::msg(format!(
            "Unhandled status code {}",
            status.as_str()
        )));
    }
    info!("Booked {}", nw_activity.name);
    Ok(())
}

#[async_recursion]
async fn run_booking(activity: BookableActivity, num_retries: u8) -> Result<()> {
    if num_retries == 0 {
        return Err(Error::msg(activity.name.clone()));
    } else {
        match find_activity_by_name(activity.clone()).await {
            Ok(_) => return Ok(()),
            Err(e) => {
                error!("{}", e.to_string());
                info!("retrying again in 1 minute");
                tokio::time::sleep(Duration::minutes(1).to_std().unwrap()).await;
                run_booking(activity, num_retries - 1).await
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct BookableActivity {
    name: String,
    cron_time: String,
    user_id: u32,
    start_time: String,
    user_name: String,
}

#[derive(Serialize, Deserialize)]
struct ConfigActivities {
    activities: Vec<BookableActivity>,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_from_env(Env::new().default_filter_or("info"));
    let rsb_config_url = env::var("RSB_CONFIG_URL").expect("no config url was provided");
    let rsb_config_api_key =
        env::var("RSB_CONFIG_API_KEY").expect("no config api key was provided");
    let client = reqwest::Client::new();

    let config_response = client
        .get(format!(
            "{}/api/config/nordic-wellness-pass.json",
            rsb_config_url
        ))
        .header(
            "Authorization",
            format!("Bearer {}", rsb_config_api_key.trim()),
        )
        .send()
        .await?
        .text()
        .await?;

    let config = serde_json::from_str::<ConfigActivities>(&config_response)?;

    for activity in config.activities {
        tokio::task::spawn(async move {
            // create a timezone instance of UTC+2 = Sweden
            let swe_tz = FixedOffset::east_opt(2 * 3600).expect("Time out of bounds");
            let schedule = Schedule::from_str(&activity.cron_time).expect(&format!(
                "unable to parse cron expression {}",
                &activity.cron_time
            ));
            let readable_schedule = get_description_cron(&activity.cron_time)
                .expect("unable to get readable cron expression");

            info!(
                "automatic booker triggering [{}] for {} ({}) and activity [{}] ",
                &readable_schedule, &activity.user_name, &activity.user_id, &activity.name,
            );

            for next_time in schedule.upcoming(swe_tz) {
                let activity = activity.clone();
                let now = Local::now().with_timezone(&swe_tz);
                let wait_time = next_time - now;
                let sleep_sec = core::time::Duration::from_secs(wait_time.num_seconds() as u64);
                let wait_time_readable = format_duration(wait_time.to_std().unwrap()).to_string();
                info!(
                    "waiting {} until next check for activity {}",
                    &wait_time_readable, &activity.name
                );
                tokio::time::sleep(sleep_sec).await;
                run_booking(activity, 15).await.expect("Unable to book!");
                tokio::time::sleep(Duration::minutes(5).to_std().unwrap()).await;
            }
        });
    }
    tokio::task::spawn_blocking(|| {
        let duration = Duration::days(365)
            .to_std()
            .expect("could not convert chrono to std time");
        std::thread::sleep(duration);
    })
    .await?;
    Ok(())
}
