#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[allow(non_upper_case_globals)]
#[export_name = "malloc_conf"]
// if you change this, also change `LG_PROF_SAMPLE`
pub static malloc_conf: &[u8] = b"prof:true,prof_active:true,lg_prof_sample:19\0";

#[tokio::main]
async fn main() {
    let mut v = vec![];
    for i in 0..1000000 {
        v.push(i);
    }

    let app = axum::Router::new()
        .route("/debug/pprof/heap", axum::routing::get(handle_get_heap));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

pub async fn handle_get_heap() -> Result<impl axum::response::IntoResponse, (axum::http::StatusCode, String)> {
    let mut prof_ctl = jemalloc_pprof::jemalloc::PROF_CTL.as_ref().unwrap().lock().await;
    require_profiling_activated(&prof_ctl)?;
    let dump_file = prof_ctl
        .dump()
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let dump_reader = std::io::BufReader::new(dump_file);
    let profile = jemalloc_pprof::jemalloc::parse_jeheap(dump_reader)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let pprof = profile.to_pprof(("inuse_space", "bytes"), ("space", "bytes"), None);
    Ok(pprof)
}

/// Checks whether jemalloc profiling is activated an returns an error response if not.
fn require_profiling_activated(prof_ctl: &jemalloc_pprof::jemalloc::JemallocProfCtl) -> Result<(), (axum::http::StatusCode, String)> {
    if prof_ctl.activated() {
        Ok(())
    } else {
        Err((axum::http::StatusCode::FORBIDDEN, "heap profiling not activated".into()))
    }
}
