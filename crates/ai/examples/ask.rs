//! Runs one request through an AI provider with AppDoctor's tools, printing
//! what happens:
//! `cargo run -p appdoctor-ai --example ask -- <claude|codex|gemini|ollama> <appdoctor binary> "<request>" [model]`
//! Uses `APPDOCTOR_DATA_DIR` like the app.
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use appdoctor_ai::{Launch, Provider, Task, detect, run};

fn main() {
    let mut args = std::env::args().skip(1);
    let provider = Provider::from_slug(&args.next().expect("provider")).expect("known provider");
    let binary = args.next().expect("appdoctor binary");
    let prompt = args.next().expect("request");
    let model = args.next();
    let found = detect().into_iter().find(|f| f.provider == provider).expect("installed");
    let data_dir = PathBuf::from(std::env::var("APPDOCTOR_DATA_DIR").expect("APPDOCTOR_DATA_DIR"));
    let task = Task {
        provider,
        path: found.path,
        model,
        prompt,
        context: "You are testing AppDoctor's tools. Answer briefly.".into(),
        launch: Launch {
            command: binary,
            args: vec!["mcp".into()],
            env: vec![("APPDOCTOR_DATA_DIR".into(), data_dir.display().to_string())],
        },
        data_dir,
        project: None,
    };
    run(&task, &mut |event| println!("{event:?}"), Arc::new(AtomicBool::new(false)));
}
