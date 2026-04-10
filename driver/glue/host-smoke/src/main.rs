unsafe extern "C" {
    fn wd_host_smoke_run() -> i32;
}

fn main() {
    let _keep_symbol =
        wd_kmdf::wd_runtime_glue_create as extern "C" fn(usize) -> *mut wd_kmdf::RuntimeGlueApi;
    let _ = _keep_symbol;

    let code = unsafe { wd_host_smoke_run() };
    if code != 0 {
        eprintln!("wd_host_smoke_run failed with code={code}");
    }
    std::process::exit(code);
}
