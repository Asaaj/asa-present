

mod coordinator_manager;
mod error;

#[macro_use]
extern crate rocket;

use std::time::Duration;

use async_mutex::Mutex;
use coordinator_manager::CoordinatorManager;
use orchestrator::coordinator;
use rocket::futures::{TryFutureExt};
use rocket::serde::json::Json;
use rocket::serde::Deserialize;
use rocket::State;
use tokio::time::sleep;

use crate::error::*;

#[derive(Clone, Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
struct CompileCodeRequest {
	source_code: String,
}

async fn do_compile(
	shared_coordinator: coordinator_manager::SharedCoordinator,
	req: CompileCodeRequest,
) -> Result<(), Error> {
	let req = coordinator::CompileRequest {
		target: coordinator::CompileTarget::Wasm,
		channel: coordinator::Channel::Stable,
		crate_type: coordinator::CrateType::Library(coordinator::LibraryType::Cdylib),
		mode: coordinator::Mode::Release,
		edition: coordinator::Edition::Rust2021,
		tests: false,
		backtrace: true,
		code: req.source_code.to_string(),
	};

	let with_output_res = shared_coordinator.compile(req).await;

	match with_output_res {
		Ok(res) => {
			println!("OUTPUT: {res:?}");
		}
		Err(e) => {
			println!("{e}");
			// e.context(CompileSnafu)?;
		}
	}
	// .context(CompileSnafu)?;

	// println!("OUTPUT: {with_output:?}");

	Ok(())
}

#[post("/compile", data = "<code_request>")]
async fn compile_code(
	code_request: Json<CompileCodeRequest>,
	manager: &State<Mutex<CoordinatorManager>>,
) {
	println!("Compile request received: {:?}", code_request);

	// TODO: Effectively sequential now
	let mut locked_manager = manager.lock().await;

	let request_inner = code_request.0.clone();
	let _spawned = locked_manager
		.spawn(move |shared_coordinator| {
			println!("BUILDING");
			do_compile(shared_coordinator, request_inner).map_err(|e| (e, None))
		})
		.await;

	if let Some(task) = locked_manager.join_next().await {
		println!("Task complete!");

		let (_error, _meta) = match task {
			Ok(Ok(())) => return,
			Ok(Err(error)) => error,
			Err(error) => {
				// The task was cancelled; no need to report
				let Ok(panic) = error.try_into_panic() else { return };

				let text = match panic.downcast::<String>() {
					Ok(text) => *text,
					Err(panic) => match panic.downcast::<&str>() {
						Ok(text) => text.to_string(),
						_ => "An unknown panic occurred".into(),
					},
				};
				(TaskPanicSnafu { text }.build(), None)
			}
		};
	}

	sleep(Duration::from_secs(5)).await;
}

#[launch]
async fn rocket() -> _ {
	rocket::build()
		.manage(Mutex::new(CoordinatorManager::new().await))
		.mount("/", routes![compile_code])
}
