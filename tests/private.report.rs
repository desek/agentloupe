//! @agents-index Integration tests: the report by token and by tag equals the fixture calls; the no-token and none groups; sessions.

#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/private.rs"]
mod private;

use private::get_json;

fn row<'a>(body: &'a serde_json::Value, group: &str) -> &'a serde_json::Value {
    body["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["group"] == group)
        .unwrap_or_else(|| panic!("no {group} in {body}"))
}

#[test]
fn group_by_token_and_tag() {
    let (l, _) = private::start_with_fixture();
    let (s, body) = get_json(&l, "/private/api/v1/report?group=token");
    assert_eq!(s, 200, "{body}");
    let laptop = row(&body, "laptop");
    assert_eq!(laptop["input_tokens"], 200);
    assert_eq!(laptop["output_tokens"], 20);
    assert_eq!(laptop["total_tokens"], 220);
    assert_eq!(laptop["calls"], 2);
    assert_eq!(laptop["sessions"], 1);
    assert_eq!(laptop["api_equivalent_cost_usd"], "0.5");
    assert_eq!(laptop["unpriced_calls"], 1);
    assert_eq!(laptop["status"], "active");
    assert_eq!(laptop["tags"]["owner"], "alice");
    assert_eq!(laptop["days"].as_array().unwrap().len(), 1);
    assert_eq!(laptop["days"][0]["total_tokens"], 220);
    assert_eq!(laptop["days"][0]["sessions"], 1);
    assert_eq!(row(&body, "ci")["api_equivalent_cost_usd"], "0.25");
    let total: i64 = body["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["total_tokens"].as_i64().unwrap())
        .sum();
    assert_eq!(total, 220 + 55 + 7);
    let (s, tag) = get_json(&l, "/private/api/v1/report?group=tag&tag_key=owner");
    assert_eq!(s, 200);
    assert_eq!(row(&tag, "alice")["total_tokens"], 220);
    assert!(row(&tag, "alice").get("status").is_none());
}

#[test]
fn no_token_and_none_groups() {
    let (l, _) = private::start_with_fixture();
    let (_, body) = get_json(&l, "/private/api/v1/report?group=token");
    assert_eq!(row(&body, "(no token)")["total_tokens"], 7);
    let (_, tag) = get_json(&l, "/private/api/v1/report?group=tag&tag_key=owner");
    assert_eq!(row(&tag, "(none)")["total_tokens"], 55);
    assert_eq!(row(&tag, "(no token)")["calls"], 1);
    assert_eq!(get_json(&l, "/private/api/v1/report?group=x").0, 400);
    assert_eq!(get_json(&l, "/private/api/v1/report?group=tag").0, 400);
    assert_eq!(
        get_json(&l, "/private/api/v1/report?group=token&from=2020-01-01").0,
        400
    );
}

#[test]
fn sessions_of_one_token() {
    let (l, _) = private::start_with_fixture();
    let (s, body) = get_json(&l, "/private/api/v1/sessions?token=laptop");
    assert_eq!(s, 200, "{body}");
    let rows = body["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["total_tokens"], 220);
    assert_eq!(rows[0]["summary"], "Fixed the parser");
    assert!(body["next"].is_null());
    let (_, none) = get_json(&l, "/private/api/v1/sessions?token=%28no%20token%29");
    assert_eq!(none["rows"][0]["total_tokens"], 7);
    assert_eq!(get_json(&l, "/private/api/v1/sessions").0, 400);
}
