#[render::component]
pub fn TimeAgo<'a>(
    since: chrono::DateTime<chrono::offset::FixedOffset>,
    lang: &'a crate::Translator,
) {
    let since_str = since.to_rfc3339();

    let duration = chrono::offset::Utc::now().signed_duration_since(since);

    let (key, args) = {
        let weeks = duration.num_weeks();
        if weeks > 52 {
            let years = ((weeks as f32) / 52.18).floor() as u32;
            (
                "timeago_years",
                Some(fluent::fluent_args!["years" => years]),
            )
        } else if weeks > 5 {
            let months = (f32::from(weeks as i8) / 4.35).floor() as u8;
            (
                "timeago_months",
                Some(fluent::fluent_args!["months" => months]),
            )
        } else if weeks > 0 {
            (
                "timeago_weeks",
                Some(fluent::fluent_args!["weeks" => weeks]),
            )
        } else {
            let days = duration.num_days();
            if days > 0 {
                ("timeago_days", Some(fluent::fluent_args!["days" => days]))
            } else {
                let hours = duration.num_hours();
                if hours > 0 {
                    (
                        "timeago_hours",
                        Some(fluent::fluent_args!["hours" => hours]),
                    )
                } else {
                    let minutes = duration.num_minutes();
                    if minutes > 0 {
                        (
                            "timeago_minutes",
                            Some(fluent::fluent_args!["minutes" => minutes]),
                        )
                    } else {
                        let seconds = duration.num_seconds();

                        if seconds > 0 {
                            (
                                "timeago_seconds",
                                Some(fluent::fluent_args!["seconds" => seconds]),
                            )
                        } else if seconds < 0 {
                            ("timeago_future", None)
                        } else {
                            ("timeago_now", None)
                        }
                    }
                }
            }
        }
    };
    let text = lang.tr(key, args.as_ref()).into_owned();

    render::rsx! {
        <span title={since_str}>{text}</span>
    }
}
