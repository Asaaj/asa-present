use snafu::prelude::*;

use crate::coordinator_manager;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub(crate) enum Error {
	#[snafu(display("Unable to deserialize request: {}", source))]
	Deserialization { source: serde_json::Error },

	#[snafu(display("Unable to find the available crates"))]
	Crates { source: orchestrator::coordinator::CratesError },

	#[snafu(display("Unable to find the available versions"))]
	Versions { source: orchestrator::coordinator::VersionsError },

	#[snafu(display("The Miri version was missing"))]
	MiriVersion,

	#[snafu(display("Unable to shutdown the coordinator"))]
	ShutdownCoordinator { source: orchestrator::coordinator::Error },

	#[snafu(display("Unable to process the evaluate request"))]
	Evaluate { source: orchestrator::coordinator::ExecuteError },

	#[snafu(display("Unable to process the compile request"))]
	Compile { source: orchestrator::coordinator::CompileError },

	#[snafu(display("Unable to process the execute request"))]
	Execute { source: orchestrator::coordinator::ExecuteError },

	#[snafu(display("Unable to process the format request"))]
	Format { source: orchestrator::coordinator::FormatError },

	#[snafu(display("Unable to process the Clippy request"))]
	Clippy { source: orchestrator::coordinator::ClippyError },

	#[snafu(display("Unable to process the Miri request"))]
	Miri { source: orchestrator::coordinator::MiriError },

	#[snafu(display("Unable to process the macro expansion request"))]
	MacroExpansion { source: orchestrator::coordinator::MacroExpansionError },

	#[snafu(display("Could not begin the execution session"))]
	Begin { source: orchestrator::coordinator::ExecuteError },

	#[snafu(display("Could not end the execution session"))]
	End { source: orchestrator::coordinator::ExecuteError },

	#[snafu(display("The operation timed out"))]
	Timeout { source: tokio::time::error::Elapsed },

	#[snafu(display("Unable to pass stdin to the active execution"))]
	StreamingCoordinatorExecuteStdin { source: tokio::sync::mpsc::error::SendError<()> },

	#[snafu(display("Unable to spawn a coordinator task"))]
	StreamingCoordinatorSpawn { source: coordinator_manager::CoordinatorManagerError },

	#[snafu(display("Unable to send result through channel: {}", text))]
	ResultChannelFailed { text: String },

	#[snafu(display("The worker panicked: {}", text))]
	TaskPanic { text: String },
}
