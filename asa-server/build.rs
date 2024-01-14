use std::path::Path;
use std::process::Command;

fn main() {
	println!("Building compiler docker containers...");
	let build_cmd = Command::new("./build.sh")
		.current_dir(&Path::new("./compiler"))
		.output()
		.expect("Failed to execute Docker build process");

	println!("Docker build status: {}", build_cmd.status);
	println!("\nDocker build stdout: \n{}", String::from_utf8_lossy(&build_cmd.stdout));
	println!("\nDocker build stderr: \n{}", String::from_utf8_lossy(&build_cmd.stderr));

	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rerun-if-changed=compiler/rust-base/");
	println!("cargo:rerun-if-changed=compiler/build.sh");

	assert!(build_cmd.status.success());
}
