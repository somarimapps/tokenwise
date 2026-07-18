use tokenwise_common::ExitCode;
use tokenwise_core::doctor::{exit_code_from_results, format_doctor_output, Doctor, DoctorPaths};

/// Run 10-layer health check and print results.
pub async fn run() -> Result<(), ExitCode> {
    let paths = DoctorPaths::default_paths();
    let doctor = Doctor::new(paths);

    let results = doctor.run_all().await;
    let output = format_doctor_output(&results);
    println!("{}", output);

    let code = exit_code_from_results(&results);
    match code {
        0 => Ok(()),
        1 => Err(ExitCode::Failure),
        _ => Err(ExitCode::Failure),
    }
}
