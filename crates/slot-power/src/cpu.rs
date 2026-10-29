use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CpuGovernor {
    Schedutil,
    Performance,
}

impl CpuGovernor {
    fn name(self) -> &'static str {
        match self {
            Self::Schedutil => "schedutil",
            Self::Performance => "performance",
        }
    }
}

/// Owns device governor selection and returns to schedutil on exit.
pub struct CpuGovernors {
    policies: Vec<PathBuf>,
    requested: Option<CpuGovernor>,
}

impl Default for CpuGovernors {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuGovernors {
    pub fn new() -> Self {
        Self::at(Path::new("/sys/devices/system/cpu/cpufreq"))
    }

    fn at(root: &Path) -> Self {
        let policies = fs::read_dir(root)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("policy"))
            .filter_map(|entry| {
                let path = entry.path().join("scaling_governor");
                fs::read_to_string(&path).ok()?;
                Some(path)
            })
            .collect();
        Self {
            policies,
            requested: None,
        }
    }

    pub fn set(&mut self, governor: CpuGovernor) {
        if self.requested == Some(governor) {
            return;
        }
        self.requested = Some(governor);
        for policy in &self.policies {
            if let Err(error) = fs::write(policy, governor.name()) {
                eprintln!(
                    "slot: CPU {}: {error} ({})",
                    governor.name(),
                    policy.display()
                );
            }
        }
    }
}

impl CpuGovernors {
    pub fn restore(&mut self) {
        self.requested = None;
        self.set(CpuGovernor::Schedutil);
    }
}

impl Drop for CpuGovernors {
    fn drop(&mut self) {
        self.restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switches_all_policies_and_returns_to_schedutil() {
        let root = tempfile::tempdir().unwrap();
        let paths: Vec<_> = ["performance", "ondemand"]
            .into_iter()
            .enumerate()
            .map(|(n, gov)| {
                let dir = root.path().join(format!("policy{n}"));
                fs::create_dir(&dir).unwrap();
                let path = dir.join("scaling_governor");
                fs::write(&path, gov).unwrap();
                path
            })
            .collect();
        {
            let mut cpu = CpuGovernors::at(root.path());
            cpu.set(CpuGovernor::Schedutil);
            for path in &paths {
                assert_eq!(fs::read_to_string(path).unwrap(), "schedutil");
            }
            cpu.set(CpuGovernor::Performance);
            for path in &paths {
                assert_eq!(fs::read_to_string(path).unwrap(), "performance");
            }
            cpu.restore();
        }
        assert_eq!(fs::read_to_string(&paths[0]).unwrap(), "schedutil");
        assert_eq!(fs::read_to_string(&paths[1]).unwrap(), "schedutil");
    }

    #[test]
    fn absent_cpufreq_is_supported() {
        let root = tempfile::tempdir().unwrap();
        let mut cpu = CpuGovernors::at(&root.path().join("absent"));
        cpu.set(CpuGovernor::Schedutil);
        cpu.set(CpuGovernor::Performance);
    }
}
