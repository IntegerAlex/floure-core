//! Floure engine: audio capture, VAD, ASR, cleanup, output, and local data.

/// The application version compiled into the binary.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_matches_workspace_package() {
        assert_eq!(super::version(), "0.3.0");
    }
}
