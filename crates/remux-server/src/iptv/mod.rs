pub mod m3u;
pub mod xmltv;
pub mod xtream;

pub use m3u::{M3uChannel, parse_m3u_stream};
pub use xmltv::{EpgProgram, parse_xmltv};
pub use xtream::{
    fetch_series_list, fetch_vod_streams, fetch_xtream_categories,
    fetch_xtream_channels,
};

use anyhow::Result;
use chrono::{NaiveDateTime, Utc};
use futures::TryStreamExt;
use tokio_util::io::{StreamReader, SyncIoBridge};
use uuid::Uuid;

use crate::{AppContext, db, db::ProgramKind};

/// Stream-parse an XMLTV EPG, match each programme to a channel by tvg_id,
/// and upsert in batches of 2000.
///
/// `epg_source_id` tags every programme this run owns. Pruning is scoped to
/// that tag, so importing a second EPG source for the same channels cannot
/// delete the programmes the first one just wrote.
pub async fn stream_import_epg(
    client: &reqwest::Client,
    url: &str,
    epg_source_id: &str,
    channels: &[(Uuid, Option<String>)],
    ctx: &AppContext,
) -> Result<usize> {
    if channels.is_empty() {
        return Ok(0);
    }

    let tvg_map: std::collections::HashMap<String, Uuid> = channels
        .iter()
        .filter_map(|(id, tvg)| {
            tvg.as_ref()
                .map(|t| (t.clone(), *id))
        })
        .collect();

    let resp = client
        .get(url)
        .send()
        .await?;
    let is_gzip = resp
        .headers()
        .get(reqwest::header::CONTENT_ENCODING)
        .and_then(|v| {
            v.to_str()
                .ok()
        })
        .map(|s| s.contains("gzip"))
        .unwrap_or(false)
        || url
            .to_lowercase()
            .ends_with(".gz");
    let byte_stream = resp
        .bytes_stream()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
    let async_reader = StreamReader::new(byte_stream);
    let handle = tokio::runtime::Handle::current();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<EpgProgram>(500);

    let parse_handle = tokio::task::spawn_blocking(move || -> Result<()> {
        let sync_reader = SyncIoBridge::new_with_handle(async_reader, handle);
        if is_gzip {
            let gz = flate2::read::GzDecoder::new(sync_reader);
            let buf_reader = std::io::BufReader::with_capacity(256 * 1024, gz);
            parse_xmltv(buf_reader, |prog| {
                tx.blocking_send(prog)
                    .ok();
            })
        } else {
            let buf_reader = std::io::BufReader::with_capacity(256 * 1024, sync_reader);
            parse_xmltv(buf_reader, |prog| {
                tx.blocking_send(prog)
                    .ok();
            })
        }
    });

    // Record time before the first upsert so that any program not re-imported
    // during this run (updated_at will be older) can be pruned at the end.
    let import_start = Utc::now().naive_utc();

    let mut batch: Vec<db::Media> = Vec::with_capacity(500);
    let mut total = 0usize;

    while let Some(prog) = rx
        .recv()
        .await
    {
        let Some(&channel_id) = tvg_map.get(&prog.channel_id) else {
            continue;
        };
        let prog_id = Uuid::new_v5(
            &channel_id,
            format!(
                "{}{}",
                prog.start
                    .map(|d| d.to_string())
                    .unwrap_or_default(),
                prog.title
            )
            .as_bytes(),
        );
        let mut media = db::Media {
            id: prog_id,
            title: prog.title,
            kind: db::MediaKind::TvProgram,
            parent_id: Some(channel_id),
            description: prog.description,
            live_start: prog.start,
            live_end: prog.end,
            program_kind: prog.program_kind,
            external_ids: db::ExternalIds {
                epg_source_id: Some(epg_source_id.to_owned()),
                ..Default::default()
            },
            ..Default::default()
        };
        if let Some(poster_url) = prog.poster {
            media.set_image(db::ImageKind::Primary, poster_url);
        }
        batch.push(media);
        total += 1;

        if batch.len() >= 500 {
            db::Media::upsert(&ctx.db, &batch).await?;
            batch.clear();
        }
    }

    if !batch.is_empty() {
        db::Media::upsert(&ctx.db, &batch).await?;
        drop(batch);
    }

    parse_handle.await??;

    // Delete programs this source imported in an earlier run but did not
    // re-import now. Programmes upserted above have updated_at >= import_start;
    // stale ones do not. Scoped by `epg_source_id` so another EPG source's
    // programmes (which may cover the same channels) survive.
    for chunk in channels.chunks(200) {
        let mut qb = sqlx::QueryBuilder::new(
            "DELETE FROM media WHERE kind = 'tv_program' AND updated_at < ",
        );
        qb.push_bind(import_start);
        qb.push(" AND (json_extract(external_ids, '$.epg_source_id') = ");
        qb.push_bind(epg_source_id);
        // Untagged rows are leftovers from imports that predate the tag:
        // no source claims them, so the first run after the upgrade clears them.
        qb.push(" OR json_extract(external_ids, '$.epg_source_id') IS NULL)");
        qb.push(" AND parent_id IN (");
        let mut sep = qb.separated(", ");
        for (id, _) in chunk {
            sep.push_bind(*id);
        }
        qb.push(")");
        qb.build()
            .execute(&ctx.db)
            .await?;
    }

    sqlx::query(
        "UPDATE media SET program_kind = (
            SELECT c.program_kind FROM media c WHERE c.id = media.parent_id
        )
        WHERE kind = 'tv_program' AND program_kind IS NULL",
    )
    .execute(&ctx.db)
    .await?;

    for chunk in channels.chunks(200) {
        let mut qb = sqlx::QueryBuilder::new(
            "DELETE FROM media WHERE kind = 'tv_program' AND live_end < datetime('now', '-1 day') AND parent_id IN (",
        );
        let mut sep = qb.separated(", ");
        for (id, _) in chunk {
            sep.push_bind(*id);
        }
        qb.push(")");
        qb.build()
            .execute(&ctx.db)
            .await?;
    }

    Ok(total)
}

/// URL of a past programme from a channel's catch-up archive.
///
/// Handles the two conventions providers publish:
///
/// * a `catchup-source` template with placeholders (`{utc}`, `{utcend}`,
///   `${start}`, `{duration}`, ...), filled from the programme's times;
/// * a "shift" URL — the live channel URL (or a template without
///   placeholders) plus `utc` and `lutc` query parameters, where `utc` is the
///   programme start and `lutc` the moment the archive window ends (now).
///
/// `None` when the channel has no archive (`catchup_days` unset or zero), the
/// programme has not aired yet, or it starts outside the archive window.
pub fn catchup_url(
    channel_url: &str,
    catchup_source: Option<&str>,
    catchup_days: Option<i64>,
    program_start: Option<NaiveDateTime>,
    program_end: Option<NaiveDateTime>,
    now: NaiveDateTime,
) -> Option<String> {
    let days = catchup_days.filter(|d| *d > 0)?;
    let start = program_start?;
    if start > now || start < now - chrono::Duration::days(days) {
        return None;
    }
    // An ongoing programme only has archive up to now.
    let end = program_end
        .unwrap_or(start)
        .min(now);
    let template = catchup_source
        .map(str::trim)
        .filter(|s| is_http_url(s))
        .or_else(|| Some(channel_url.trim()).filter(|s| is_http_url(s)))?;

    match fill_catchup_template(template, start, end, now) {
        Some(url) => Some(url),
        None => append_catchup_window(template, start, now),
    }
}

fn is_http_url(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|u| matches!(u.scheme(), "http" | "https"))
}

/// Replace the placeholders a catch-up template may contain. Returns `None`
/// when the template has none, which means the provider expects the
/// `utc`/`lutc` query parameters instead.
fn fill_catchup_template(
    template: &str,
    start: NaiveDateTime,
    end: NaiveDateTime,
    now: NaiveDateTime,
) -> Option<String> {
    let start_fmt = |fmt: &str| {
        start
            .format(fmt)
            .to_string()
    };
    let values: [(&str, String); 11] = [
        (
            "utc",
            start
                .and_utc()
                .timestamp()
                .to_string(),
        ),
        (
            "utcend",
            end.and_utc()
                .timestamp()
                .to_string(),
        ),
        (
            "lutc",
            now.and_utc()
                .timestamp()
                .to_string(),
        ),
        (
            "duration",
            (end - start)
                .num_minutes()
                .to_string(),
        ),
        ("start", start_fmt("%Y%m%d%H%M%S")),
        (
            "end",
            end.format("%Y%m%d%H%M%S")
                .to_string(),
        ),
        (
            "timestamp",
            start
                .and_utc()
                .timestamp()
                .to_string(),
        ),
        ("Y", start_fmt("%Y")),
        ("m", start_fmt("%m")),
        ("d", start_fmt("%d")),
        ("H", start_fmt("%H")),
    ];

    let mut out = template.to_owned();
    let mut replaced = false;
    // `${token}` first: replacing `{token}` first would leave a stray `$`.
    for prefix in ["${", "{"] {
        for (token, value) in &values {
            let needle = format!("{prefix}{token}}}");
            if out.contains(&needle) {
                out = out.replace(&needle, value);
                replaced = true;
            }
        }
    }
    replaced.then_some(out)
}

/// Append the archive window as query parameters, replacing any `utc`/`lutc`
/// the provider's URL already carries.
fn append_catchup_window(
    url: &str,
    start: NaiveDateTime,
    now: NaiveDateTime,
) -> Option<String> {
    let mut parsed = url::Url::parse(url).ok()?;
    let kept: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(key, _)| key != "utc" && key != "lutc")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    parsed.set_query(None);
    {
        let mut query = parsed.query_pairs_mut();
        for (key, value) in kept {
            query.append_pair(&key, &value);
        }
        query.append_pair(
            "utc",
            &start
                .and_utc()
                .timestamp()
                .to_string(),
        );
        query.append_pair(
            "lutc",
            &now.and_utc()
                .timestamp()
                .to_string(),
        );
    }
    Some(parsed.into())
}

/// Map a free-text category/group string to a `ProgramKind`.
/// Used for both XMLTV `<category>` tags and M3U/Xtream group-title values.
pub fn parse_program_kind(category: &str) -> Option<ProgramKind> {
    let lower = category.to_lowercase();
    let rules: &[(&[&str], ProgramKind)] = &[
        (
            &[
                "movie", "film", "cinema", "cine", "vod", "pelicul", "filme", "kino",
            ],
            ProgramKind::Movie,
        ),
        (
            &[
                "series",
                "episode",
                "soap",
                "sitcom",
                "show",
                "telenovela",
                "serial",
                "miniseries",
            ],
            ProgramKind::Series,
        ),
        (
            &[
                "news",
                "info",
                "actualit",
                "journalism",
                "documentary",
                "current affairs",
                "noticias",
            ],
            ProgramKind::News,
        ),
        (
            &[
                "children", "kids", "youth", "cartoon", "enfant", "jeunesse", "family",
                "disney", "infantil", "kinder",
            ],
            ProgramKind::Kids,
        ),
        (
            &[
                "sport",
                "basketball",
                "baseball",
                "football",
                "soccer",
                "tennis",
                "cricket",
                "golf",
                "rugby",
                "hockey",
                "racing",
                "boxing",
                "wrestling",
                "fighting",
                "mma",
            ],
            ProgramKind::Sports,
        ),
    ];
    rules
        .iter()
        .find(|(terms, _)| {
            terms
                .iter()
                .any(|t| lower.contains(t))
        })
        .map(|(_, kind)| kind.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    fn at(seconds_from_epoch: i64) -> NaiveDateTime {
        Utc.timestamp_opt(seconds_from_epoch, 0)
            .unwrap()
            .naive_utc()
    }

    #[test]
    fn a_shift_channel_appends_the_archive_window() {
        let now = at(1_700_000_000);
        let start = now - Duration::hours(2);
        let url = catchup_url(
            "http://provider/live/user/pass/1.ts",
            None,
            Some(7),
            Some(start),
            Some(start + Duration::hours(1)),
            now,
        )
        .unwrap();
        let parsed = url::Url::parse(&url).unwrap();
        let params: std::collections::HashMap<_, _> = parsed
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        assert_eq!(
            params["utc"],
            start
                .and_utc()
                .timestamp()
                .to_string()
        );
        assert_eq!(
            params["lutc"],
            now.and_utc()
                .timestamp()
                .to_string()
        );
    }

    #[test]
    fn a_template_fills_the_programme_window() {
        let now = at(1_700_000_000);
        let start = now - Duration::hours(2);
        let end = start + Duration::minutes(45);
        let url = catchup_url(
            "http://provider/live/1.ts",
            Some("http://cdn/archive?start={utc}&end={utcend}&d={duration}"),
            Some(7),
            Some(start),
            Some(end),
            now,
        )
        .unwrap();
        assert_eq!(
            url,
            format!(
                "http://cdn/archive?start={}&end={}&d=45",
                start
                    .and_utc()
                    .timestamp(),
                end.and_utc()
                    .timestamp()
            )
        );
    }

    #[test]
    fn a_template_understands_xtream_style_placeholders() {
        let now = at(1_700_000_000);
        let start = now - Duration::hours(2);
        let end = start + Duration::minutes(30);
        let url = catchup_url(
            "http://provider/live/1.ts",
            Some("http://cdn/${start}/${end}/{Y}{m}{d}/{H}"),
            Some(7),
            Some(start),
            Some(end),
            now,
        )
        .unwrap();
        assert_eq!(
            url,
            format!(
                "http://cdn/{}/{}/{}/{}",
                start.format("%Y%m%d%H%M%S"),
                end.format("%Y%m%d%H%M%S"),
                start.format("%Y%m%d"),
                start.format("%H"),
            )
        );
    }

    #[test]
    fn an_ongoing_programme_stops_at_now() {
        let now = at(1_700_000_000);
        let start = now - Duration::minutes(10);
        let url = catchup_url(
            "http://provider/live/1.ts",
            Some("http://cdn/archive?end={utcend}"),
            Some(7),
            Some(start),
            Some(now + Duration::hours(1)),
            now,
        )
        .unwrap();
        assert_eq!(
            url,
            format!(
                "http://cdn/archive?end={}",
                now.and_utc()
                    .timestamp()
            )
        );
    }

    #[test]
    fn no_archive_window_means_no_catchup() {
        let now = at(1_700_000_000);
        let start = now - Duration::hours(1);
        assert!(
            catchup_url(
                "http://provider/live/1.ts",
                None,
                None,
                Some(start),
                Some(now),
                now
            )
            .is_none()
        );
        assert!(
            catchup_url(
                "http://provider/live/1.ts",
                None,
                Some(0),
                Some(start),
                Some(now),
                now
            )
            .is_none()
        );
    }

    #[test]
    fn a_programme_outside_the_window_is_not_playable() {
        let now = at(1_700_000_000);
        assert!(
            catchup_url(
                "http://provider/live/1.ts",
                None,
                Some(2),
                Some(now - Duration::days(3)),
                Some(now - Duration::days(3) + Duration::hours(1)),
                now
            )
            .is_none(),
            "an expired programme"
        );
        assert!(
            catchup_url(
                "http://provider/live/1.ts",
                None,
                Some(2),
                Some(now + Duration::hours(1)),
                Some(now + Duration::hours(2)),
                now
            )
            .is_none(),
            "a programme that has not started"
        );
    }

    #[test]
    fn a_non_http_url_is_not_a_catchup_source() {
        let now = at(1_700_000_000);
        assert!(
            catchup_url(
                "/relative/live/1.ts",
                Some("rtmp://cdn/archive"),
                Some(7),
                Some(now - Duration::hours(1)),
                Some(now),
                now
            )
            .is_none()
        );
    }

    #[tokio::test]
    async fn a_second_guide_keeps_the_first_ones_programmes() {
        let (_server, guard) = crate::integration_test::new_test_server()
            .await
            .unwrap();
        let ctx = &guard.0;
        let channel_id = Uuid::new_v4();
        db::Media {
            id: channel_id,
            title: "Channel 1".into(),
            kind: db::MediaKind::TvChannel,
            tvg_id: Some("ch1".into()),
            enabled: true,
            ..Default::default()
        }
        .save(&ctx.db)
        .await
        .unwrap();
        let channels = vec![(channel_id, Some("ch1".to_string()))];

        let now = Utc::now().naive_utc();
        let show = |title: &str, hours: i64| {
            let start = now + Duration::hours(hours);
            let end = start + Duration::minutes(30);
            format!(
                "<?xml version=\"1.0\"?><tv><programme channel=\"ch1\" start=\"{}\" stop=\"{}\"><title>{title}</title></programme></tv>",
                start.format("%Y%m%d%H%M%S"),
                end.format("%Y%m%d%H%M%S"),
            )
        };
        let provider = httpmock::MockServer::start();
        for (path, body) in [
            ("/a1.xml", show("Guide A one", 1)),
            ("/a2.xml", show("Guide A two", 3)),
            ("/b.xml", show("Guide B", 2)),
        ] {
            provider.mock(|when, then| {
                when.path(path);
                then.status(200)
                    .body(body);
            });
        }
        let client = reqwest::Client::new();
        async fn import(
            client: &reqwest::Client,
            provider: &httpmock::MockServer,
            path: &str,
            source: &str,
            channels: &[(Uuid, Option<String>)],
            ctx: &crate::AppContext,
        ) -> usize {
            let url = format!("{}{}", provider.base_url(), path);
            stream_import_epg(client, &url, source, channels, ctx)
                .await
                .unwrap()
        }
        let titles = || async {
            sqlx::query_scalar::<_, String>(
                "SELECT title FROM media WHERE kind = 'tv_program' AND parent_id = ? \
                 ORDER BY title",
            )
            .bind(channel_id)
            .fetch_all(&ctx.db)
            .await
            .unwrap()
        };

        assert_eq!(
            import(&client, &provider, "/a1.xml", "epg-a", &channels, ctx).await,
            1
        );
        assert_eq!(
            import(&client, &provider, "/b.xml", "epg-b", &channels, ctx).await,
            1
        );
        assert_eq!(
            titles().await,
            vec!["Guide A one".to_string(), "Guide B".to_string()],
            "the two guides must coexist"
        );

        // The first guide now covers a different programme: only its own
        // previous row is pruned.
        assert_eq!(
            import(&client, &provider, "/a2.xml", "epg-a", &channels, ctx).await,
            1
        );
        assert_eq!(
            titles().await,
            vec!["Guide A two".to_string(), "Guide B".to_string()],
        );
    }
}
