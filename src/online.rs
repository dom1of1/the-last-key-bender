use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use eframe::egui;
use serde::Deserialize;

use crate::library::MAX_FILE_BYTES;

const SITE: &str = "https://bitmidi.com";
const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub struct Hit {
    pub title: String,
    url: String,
}

pub enum Message {
    Results(Result<Vec<Hit>, String>),
    Song(String, Result<Vec<u8>, String>),
}

/// Searches BitMidi on a background thread and sends the results back.
pub fn search(query: String, send: Sender<Message>, ctx: egui::Context) {
    thread::spawn(move || {
        let _ = send.send(Message::Results(fetch_results(&query)));
        ctx.request_repaint();
    });
}

/// Downloads a search result on a background thread and sends the file back.
pub fn download(hit: Hit, send: Sender<Message>, ctx: egui::Context) {
    thread::spawn(move || {
        let bytes = agent()
            .get(&hit.url)
            .call()
            .and_then(|mut r| {
                r.body_mut()
                    .with_config()
                    .limit(MAX_FILE_BYTES)
                    .read_to_vec()
            })
            .map_err(describe);
        let _ = send.send(Message::Song(hit.title, bytes));
        ctx.request_repaint();
    });
}

fn fetch_results(query: &str) -> Result<Vec<Hit>, String> {
    let body = agent()
        .get(format!("{SITE}/api/midi/search"))
        .query("q", query)
        .call()
        .and_then(|mut r| r.body_mut().read_to_string())
        .map_err(describe)?;
    parse_results(&body)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build()
        .into()
}

fn parse_results(body: &str) -> Result<Vec<Hit>, String> {
    #[derive(Deserialize)]
    struct Response {
        result: Results,
    }
    #[derive(Deserialize)]
    struct Results {
        results: Vec<Item>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Item {
        name: String,
        download_url: String,
    }

    let response: Response =
        serde_json::from_str(body).map_err(|_| "BitMidi sent back something unexpected")?;
    Ok(response
        .result
        .results
        .into_iter()
        .map(|item| Hit {
            title: item
                .name
                .strip_suffix(".mid")
                .unwrap_or(&item.name)
                .to_string(),
            url: format!("{SITE}{}", item.download_url),
        })
        .collect())
}

fn describe(error: ureq::Error) -> String {
    match error {
        ureq::Error::Io(_)
        | ureq::Error::HostNotFound
        | ureq::Error::ConnectionFailed
        | ureq::Error::Timeout(_) => {
            "Couldn't reach BitMidi. Check your internet connection.".into()
        }
        ureq::Error::BodyExceedsLimit(_) => "That song is too large to download".into(),
        e => format!("BitMidi request failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_results() {
        let body = r#"{"result":{"query":{"q":"x"},"results":[
            {"id":1,"name":"Gravity Falls - Theme.mid","downloadUrl":"/uploads/1.mid","plays":5}
        ]}}"#;
        let hits = parse_results(body).unwrap();
        assert_eq!(hits[0].title, "Gravity Falls - Theme");
        assert_eq!(hits[0].url, "https://bitmidi.com/uploads/1.mid");
    }

    #[test]
    fn rejects_unexpected_responses() {
        assert!(parse_results("<html>").is_err());
    }
}
