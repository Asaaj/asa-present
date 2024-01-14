#![deny(rust_2018_idioms)]

pub mod coordinator;
mod message;
pub mod worker;

pub trait DropErrorDetailsExt<T> {
	fn drop_error_details(self) -> Result<T, tokio::sync::mpsc::error::SendError<()>>;
}

impl<T, E> DropErrorDetailsExt<T> for Result<T, tokio::sync::mpsc::error::SendError<E>> {
	fn drop_error_details(self) -> Result<T, tokio::sync::mpsc::error::SendError<()>> {
		self.map_err(|_| tokio::sync::mpsc::error::SendError(()))
	}
}

fn bincode_input_closed<T>(coordinator_msg: &bincode::Result<T>) -> bool {
	if let Err(e) = coordinator_msg {
		if let bincode::ErrorKind::Io(e) = &**e {
			return e.kind() == std::io::ErrorKind::UnexpectedEof;
		}
	}

	false
}
