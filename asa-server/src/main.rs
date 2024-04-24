#[macro_use]
extern crate rocket;

use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{fs, io};

use async_channel::{unbounded, Receiver, Sender};
use async_mutex::Mutex;
use coordinator_manager::CoordinatorManager;
use glob::glob;
use orchestrator::coordinator;
use orchestrator::coordinator::{CompileResponse, CompiledCode, WithOutput};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::fs::{relative, FileServer, Options};
use rocket::futures::TryFutureExt;
use rocket::http::Header;
use rocket::serde::json::Json;
use rocket::{Request, Response, State};
use serde::{Deserialize, Serialize};
use tar::Archive;
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
	package_name: String,
	language: ProgrammingLanguage,
}

#[derive(Clone, Debug, Serialize)]
struct CompileSuccess {
	result: Vec<u8>,
	stdout: String,
	stderr: String,
}

#[derive(Clone, Debug, Serialize)]
struct TextResponseSuccess {
	result: String,
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

	#[response(status = 200)]
	TextSuccess(JsonResponse<TextResponseSuccess>),

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
	request_index: usize,
) -> Result<(), Error> {
	let package_name = req.package_name.clone();
	let req = coordinator::CompileRequest {
		target: coordinator::CompileTarget::Wasm,
		language: req.language.into(),
		crate_type: coordinator::CrateType::Library(coordinator::LibraryType::Cdylib),
		mode: coordinator::Mode::Release,
		code: req.source_code.to_string(),
		package_name: req.package_name,
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
				} => extract_code_response(code, package_name, request_index, stdout, stderr),
				// other => {CompileCodeResponse::InternalError(format!("Unknown problem with
				// compile: {other:?}")) }
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

fn try_unarchiving(result: &[u8], output_location: PathBuf) -> io::Result<()> {
	let mut archive = Archive::new(result);
	archive.unpack(output_location)
}

fn get_js_glue_file_name(package_location: PathBuf) -> Result<String, String> {
	if !package_location.exists() {
		return Err(format!("Package at {package_location:?} does not exist"));
	}

	let glob_path = package_location.join("*.js");

	let js_glob = glob_path.to_str().ok_or("Somehow failed to make glob string".to_string())?;

	let js_files: Vec<_> = glob(js_glob)
		.map_err(|_| format!("No javascript files found in {package_location:?}"))?
		.collect();

	if js_files.len() != 1 {
		return Err(format!("Expected exactly 1 javascript file, but found {}", js_files.len()));
	}

	let js_file = js_files.into_iter().nth(0)
		.ok_or(format!("Something terrible happened in the package directory {package_location:?}. Maybe a race condition?"))?
		.map_err(|_| format!("Problem retrieving the glob result for {package_location:?}"))?;

	let js_file_str =
		js_file.to_str().ok_or(format!("Failed to convert PathBuf {js_file:?} to str"))?;

	Ok(js_file_str.into())
}
fn get_js_glue_file(package_location: PathBuf) -> Result<String, String> {
	let js_file = get_js_glue_file_name(package_location)?;

	let result = fs::read_to_string(PathBuf::from(&js_file).clone())
		.map_err(|_| format!("Failed to read file {js_file:?}"))?;

	Ok(result)
}

fn extract_code_response(
	code: CompiledCode,
	package_name: String,
	request_index: usize,
	stdout: String,
	stderr: String,
) -> CompileCodeResponse {
	if let CompiledCode::CodeBin(result) = code {
		let output_location = format!("./pkg/{package_name}");
		let output_location = Path::new(output_location.as_str());

		let _ = fs::remove_dir_all(output_location);

		if let Ok(_) = try_unarchiving(&result, output_location.into()) {
			match get_js_glue_file_name(output_location.into()) {
				Ok(result) => CompileCodeResponse::TextSuccess(
					TextResponseSuccess { result, stdout, stderr }.into(),
				),
				Err(response) => CompileCodeResponse::InternalError(response),
			}
		} else {
			// TODO: This is a bad fallback...
			CompileCodeResponse::Success(CompileSuccess { result, stdout, stderr }.into())
		}
	} else if let CompiledCode::CodeStr(result) = code {
		CompileCodeResponse::TextSuccess(TextResponseSuccess { result, stdout, stderr }.into())
	} else {
		CompileCodeResponse::InternalError(format!(
			"Received unknown data type after compile: {code:?}"
		))
	}
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
	counter: &State<AtomicUsize>,
) -> CompileCodeResponse {
	let current_request = counter.fetch_add(1, Ordering::Relaxed);
	println!("Compile request {} received: {:?}", current_request, code_request);

	// TODO: Effectively sequential now
	let mut locked_manager = manager.lock().await;

	let (sender, receiver): (Sender<CompileCodeResponse>, Receiver<CompileCodeResponse>) =
		unbounded();

	let request_inner = code_request.0.clone();
	let _spawned = locked_manager
		.spawn(move |shared_coordinator| {
			do_compile(shared_coordinator, request_inner, sender, current_request)
		})
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
		.manage(AtomicUsize::new(0))
		.attach(CORS)
		.mount("/", routes![compile_code])
		.mount("/pkg", FileServer::new(relative!("../pkg"), Options::None | Options::Missing))
}
