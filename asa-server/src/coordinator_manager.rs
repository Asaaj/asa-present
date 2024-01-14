use std::future::Future;
use std::sync::Arc;

use orchestrator::coordinator::{self, Coordinator, DockerBackend};

use rocket::tokio::sync::Semaphore;
use rocket::tokio::task::JoinSet;
use snafu::{OptionExt, ResultExt, Snafu};
use tokio::task::AbortHandle;
use tracing::instrument::Instrument;

use crate::error::Error;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaInner {
	sequence_number: i64,
}

type Meta = Arc<MetaInner>;
type TaggedError = (Error, Option<Meta>);
pub type SharedCoordinator = Arc<Coordinator<DockerBackend>>;

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum CoordinatorManagerError {
	#[snafu(display("The coordinator is still referenced and cannot be idled"))]
	OutstandingCoordinatorIdle,

	#[snafu(display("Could not idle the coordinator"))]
	Idle { source: coordinator::Error },

	#[snafu(display("The coordinator is still referenced and cannot be shut down"))]
	OutstandingCoordinatorShutdown,

	#[snafu(display("Could not shut down the coordinator"))]
	Shutdown { source: coordinator::Error },
}

type CoordinatorManagerResult<T, E = CoordinatorManagerError> = Result<T, E>;

pub struct CoordinatorManager {
	coordinator: SharedCoordinator,
	tasks: JoinSet<Result<(), TaggedError>>,
	semaphore: Arc<Semaphore>,
	abort_handle: Option<AbortHandle>,
}

impl CoordinatorManager {
	const N_PARALLEL: usize = 2;

	pub async fn new() -> Self {
		Self {
			coordinator: Arc::new(Coordinator::new_docker().await),
			tasks: Default::default(),
			semaphore: Arc::new(Semaphore::new(Self::N_PARALLEL)),
			abort_handle: None,
		}
	}

	pub fn is_empty(&self) -> bool { self.tasks.is_empty() }

	pub async fn join_next(
		&mut self,
	) -> Option<Result<Result<(), TaggedError>, tokio::task::JoinError>> {
		self.tasks.join_next().await
	}

	pub async fn spawn<F, Fut>(&mut self, handler: F) -> CoordinatorManagerResult<()>
	where
		F: FnOnce(SharedCoordinator) -> Fut,
		F: 'static + Send,
		Fut: Future<Output = Result<(), TaggedError>>,
		Fut: 'static + Send,
	{
		let coordinator = self.coordinator.clone();
		let semaphore = self.semaphore.clone();

		let new_abort_handle = self.tasks.spawn(
			async move {
				let _permit = semaphore.acquire().await;
				handler(coordinator).await
			}
			.in_current_span(),
		);

		let old_abort_handle = self.abort_handle.replace(new_abort_handle);

		if let Some(abort_handle) = old_abort_handle {
			abort_handle.abort();
		}

		Ok(())
	}

	pub async fn shutdown(mut self) -> CoordinatorManagerResult<()> {
		use coordinator_manager_error::*;

		self.tasks.shutdown().await;
		Arc::into_inner(self.coordinator)
			.context(OutstandingCoordinatorShutdownSnafu)?
			.shutdown()
			.await
			.context(ShutdownSnafu)?;

		Ok(())
	}
}
