use anyhow::Result;
#[cfg(debug_assertions)]
use anyhow::bail;

/// Debug-only fault injection used by synthetic integration tests. Release
/// binaries ignore this variable entirely.
pub fn check(_point: &str) -> Result<()> {
    #[cfg(debug_assertions)]
    if std::env::var("CODEX_REHOME_FAULT")
        .is_ok_and(|value| value.split(',').any(|configured| configured == _point))
    {
        bail!("injected fault at {_point}")
    }
    Ok(())
}
