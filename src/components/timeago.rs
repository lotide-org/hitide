use crate::lang;

#[render::component]
pub fn TimeAgo<'a>(
    since: chrono::DateTime<chrono::offset::FixedOffset>,
    lang: &'a crate::Translator,
) {
    let since_str = since.to_rfc3339();

    let duration = chrono::offset::Utc::now().signed_duration_since(since);

    let arg = {
        let weeks = duration.num_weeks();
        if weeks > 52 {
            let years = ((weeks as f32) / 52.18).floor() as u32;
            lang::timeago_years(years)
        } else if weeks > 5 {
            let months = (f32::from(weeks as i8) / 4.35).floor() as u8;
            lang::timeago_months(months)
        } else if weeks > 0 {
            lang::timeago_weeks(weeks)
        } else {
            let days = duration.num_days();
            if days > 0 {
                lang::timeago_days(days)
            } else {
                let hours = duration.num_hours();
                if hours > 0 {
                    lang::timeago_hours(hours)
                } else {
                    let minutes = duration.num_minutes();
                    if minutes > 0 {
                        lang::timeago_minutes(minutes)
                    } else {
                        let seconds = duration.num_seconds();

                        if seconds > 0 {
                            lang::timeago_seconds(seconds)
                        } else if seconds < 0 {
                            lang::timeago_future()
                        } else {
                            lang::timeago_now()
                        }
                    }
                }
            }
        }
    };
    let text = lang.tr(&arg).into_owned();

    render::rsx! {
        <span title={since_str}>{text}</span>
    }
}
