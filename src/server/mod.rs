use serde::Serialize;
use warp::Filter;
use warp::filters::BoxedFilter;
use warp::http::StatusCode;
use warp::reply::Reply;

#[derive(Debug, Serialize)]
struct TestData {
    keys: &'static [&'static str],
    rows: &'static [&'static [u32]],
}

fn build_data() -> TestData {
    TestData {
        keys: &["A", "B", "C"],
        rows: &[&[1, 2, 3], &[3, 3, 2], &[2, 3, 0]],
    }
}

pub fn build_routes() -> BoxedFilter<(impl Reply,)> {
    let test_data = warp::path("test_data")
        .and(warp::path::end())
        .and(warp::get())
        .map(|| {
            let data = build_data();
            warp::reply::json(&data)
        });
    let api = warp::path("api").and(test_data);

    let assets = warp::path("assets").and(warp::fs::dir("web/assets"));
    let home = warp::path::end().and(warp::fs::file("web/index.html"));
    let missing = warp::any()
        .map(warp::reply)
        .map(|r| warp::reply::with_status(r, StatusCode::NOT_FOUND));

    home.or(assets).or(api).or(missing).boxed()
}
