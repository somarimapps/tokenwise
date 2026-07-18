/// Exit code contract for the tokenwise CLI.
///
/// | Code | Meaning                                                    |
/// |------|------------------------------------------------------------|
/// | 0    | Success — all checks passed or all repairs completed       |
/// | 1    | Failure — at least one component failed or error unresolved|
/// | 2    | Invalid invocation — unknown command or missing argument   |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    Success = 0,
    Failure = 1,
    InvalidInvocation = 2,
}

impl ExitCode {
    /// Convert to the integer code that `std::process::exit` expects.
    pub fn into_code(self) -> i32 {
        self as i32
    }

    /// Terminate the process with this exit code (does not return).
    pub fn exit(self) -> ! {
        std::process::exit(self.into_code())
    }
}

impl From<i32> for ExitCode {
    fn from(code: i32) -> Self {
        match code {
            0 => Self::Success,
            2 => Self::InvalidInvocation,
            _ => Self::Failure,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// test::cli::exit_codes_contract
    #[test]
    fn exit_codes_map_to_correct_integers() {
        assert_eq!(ExitCode::Success.into_code(), 0);
        assert_eq!(ExitCode::Failure.into_code(), 1);
        assert_eq!(ExitCode::InvalidInvocation.into_code(), 2);
    }

    #[test]
    fn from_i32_round_trips() {
        assert_eq!(ExitCode::from(0), ExitCode::Success);
        assert_eq!(ExitCode::from(1), ExitCode::Failure);
        assert_eq!(ExitCode::from(2), ExitCode::InvalidInvocation);
        assert_eq!(ExitCode::from(99), ExitCode::Failure);
    }
}
