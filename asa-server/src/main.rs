#[macro_use]
extern crate rocket;

use async_channel::{unbounded, Receiver, Sender};
use async_mutex::Mutex;
use coordinator_manager::CoordinatorManager;
use orchestrator::coordinator;
use orchestrator::coordinator::{CompileResponse, CompiledCode, WithOutput};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::futures::TryFutureExt;
use rocket::http::Header;
use rocket::serde::json::Json;
use rocket::{Request, Response, State};
use serde::{Deserialize, Serialize};
use tokio::task::JoinError;

use crate::error::*;

mod coordinator_manager;
mod error;

#[derive(Copy, Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProgrammingLanguage {
	Rust,
	Cpp,
}
impl From<ProgrammingLanguage> for coordinator::Language {
	fn from(value: ProgrammingLanguage) -> Self {
		match value {
			ProgrammingLanguage::Rust => coordinator::RustSpec::new(
				coordinator::RustChannel::Stable,
				coordinator::RustEdition::Rust2021,
			)
			.into(),
			ProgrammingLanguage::Cpp => {
				coordinator::CppSpec::new(coordinator::CppVersion::Cpp20).into()
			}
		}
	}
}

#[derive(Clone, Debug, Deserialize)]
struct CompileCodeRequest {
	source_code: String,
	language: ProgrammingLanguage,
}

#[derive(Clone, Debug, Serialize)]
struct CompileSuccess {
	result: Vec<u8>,
	stdout: String,
	stderr: String,
}

#[derive(Clone, Debug, Serialize)]
struct CompileFailed {
	exit_detail: String,
	stdout: String,
	stderr: String,
}

#[derive(Clone, Debug, Responder)]
struct JsonResponse<T: Serialize> {
	payload: Json<T>,
}

impl<T: Serialize> From<T> for JsonResponse<T> {
	fn from(value: T) -> Self { Self { payload: Json::from(value) } }
}

#[derive(Clone, Debug, Responder)]
enum CompileCodeResponse {
	#[response(status = 200)]
	Success(JsonResponse<CompileSuccess>),

	#[response(status = 400)]
	CompileError(JsonResponse<CompileFailed>),

	#[response(status = 500)]
	InternalError(String),

	#[response(status = 200)] // TODO
	CompileCancelled(String),
}

async fn do_compile(
	shared_coordinator: coordinator_manager::SharedCoordinator,
	req: CompileCodeRequest,
	sender: Sender<CompileCodeResponse>,
) -> Result<(), Error> {
	let req = coordinator::CompileRequest {
		target: coordinator::CompileTarget::Wasm,
		language: req.language.into(),
		crate_type: coordinator::CrateType::Library(coordinator::LibraryType::Cdylib),
		mode: coordinator::Mode::Release,
		code: req.source_code.to_string(),
	};

	let with_output_res = shared_coordinator.compile(req).await;

	let response: CompileCodeResponse = match with_output_res {
		Ok(res) => {
			match res {
				WithOutput {
					response: CompileResponse { success: false, exit_detail, .. },
					stdout,
					stderr,
				} => CompileCodeResponse::CompileError(
					CompileFailed { exit_detail, stdout, stderr }.into(),
				),
				WithOutput {
					response: CompileResponse { success: true, code, .. },
					stdout,
					stderr,
				} => {
					if let CompiledCode::CodeBin(result) = code {
						CompileCodeResponse::Success(
							CompileSuccess { result, stdout, stderr }.into(),
						)
					} else {
						CompileCodeResponse::InternalError(format!(
							"Received string instead of binary after compile: {code:?}"
						))
					}
				} /* other => {
				   * 	CompileCodeResponse::InternalError(format!("Unknown problem with compile:
				   * {other:?}")) } */
			}
		}
		Err(e) => {
			println!("{e}");
			CompileCodeResponse::InternalError(format!("Unknown problem with compile: {e:?}"))
		}
	};
	sender
		.send(response)
		.await
		.map_err(|err| ResultChannelFailedSnafu { text: format!("{err}") }.build())?;

	Ok(())
}

fn handle_task_panic(task: Result<Result<(), Error>, JoinError>) -> Result<(), Error> {
	return match task {
		Ok(Ok(())) => Ok(()),
		Ok(Err(error)) => Err(error),
		Err(error) => {
			// The task was cancelled; no need to report
			let Ok(panic) = error.try_into_panic() else { return Ok(()) };

			let text = match panic.downcast::<String>() {
				Ok(text) => *text,
				Err(panic) => match panic.downcast::<&str>() {
					Ok(text) => text.to_string(),
					_ => "An unknown panic occurred".into(),
				},
			};
			Err(TaskPanicSnafu { text }.build())
		}
	};
}

#[post("/compile", data = "<code_request>")]
async fn compile_code(
	code_request: Json<CompileCodeRequest>,
	manager: &State<Mutex<CoordinatorManager>>,
) -> CompileCodeResponse {
	println!("Compile request received: {:?}", code_request);

	// TODO: Effectively sequential now
	let mut locked_manager = manager.lock().await;

	let (sender, receiver): (Sender<CompileCodeResponse>, Receiver<CompileCodeResponse>) =
		unbounded();

	let request_inner = code_request.0.clone();
	let _spawned = locked_manager
		.spawn(move |shared_coordinator| do_compile(shared_coordinator, request_inner, sender))
		.await;

	let response = {
		if let Some(task) = locked_manager.join_next().await {
			println!("Task complete!");
			match handle_task_panic(task) {
				Ok(()) => receiver.recv().await.unwrap_or_else(|err| {
					CompileCodeResponse::InternalError(format!(
						"Failed to receive result from channel: {err:?}"
					))
				}),
				Err(error) => {
					CompileCodeResponse::InternalError(format!("Task panic occurred: {error:?}"))
				}
			}
		} else {
			CompileCodeResponse::InternalError("No compile task to await! Not sure how...".into())
		}
	};
	response
}

pub struct CORS;

#[rocket::async_trait]
impl Fairing for CORS {
	fn info(&self) -> Info { Info { name: "Add CORS headers to responses", kind: Kind::Response } }

	async fn on_response<'r>(&self, _request: &'r Request<'_>, response: &mut Response<'r>) {
		response.set_header(Header::new("Access-Control-Allow-Origin", "*"));
		response
			.set_header(Header::new("Access-Control-Allow-Methods", "POST, GET, PATCH, OPTIONS"));
		response.set_header(Header::new("Access-Control-Allow-Headers", "*"));
		response.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
	}
}

#[launch]
async fn rocket() -> _ {
	rocket::build()
		.manage(Mutex::new(CoordinatorManager::new().await))
		.attach(CORS)
		.mount("/", routes![compile_code])
}
