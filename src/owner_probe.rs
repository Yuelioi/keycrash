use crate::detector::Shortcut;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnerProbeResult {
    Found {
        pid: u32,
        thread_id: u32,
        path: String,
        suppressed: bool,
        architecture: String,
    },
    NotFound,
    Error(String),
}

pub fn locate_owner(shortcut: &Shortcut) -> OwnerProbeResult {
    let directory = match executable_directory() {
        Ok(path) => path,
        Err(error) => return OwnerProbeResult::Error(error),
    };

    let x64 = run_helper(&directory.join("keycrash-owner-probe.exe"), shortcut, "x64");
    if matches!(x64, OwnerProbeResult::Found { .. }) {
        return x64;
    }

    let x86_helper = directory.join("owner-x86").join("keycrash-owner-probe.exe");
    let x86 = if x86_helper.is_file() {
        run_helper(&x86_helper, shortcut, "x86")
    } else {
        OwnerProbeResult::NotFound
    };
    if matches!(x86, OwnerProbeResult::Found { .. }) {
        return x86;
    }

    match (x64, x86) {
        (OwnerProbeResult::Error(first), OwnerProbeResult::Error(second)) => {
            OwnerProbeResult::Error(format!("x64：{first}；x86：{second}"))
        }
        (OwnerProbeResult::Error(error), _) | (_, OwnerProbeResult::Error(error)) => {
            OwnerProbeResult::Error(error)
        }
        _ => OwnerProbeResult::NotFound,
    }
}

fn run_helper(helper: &PathBuf, shortcut: &Shortcut, architecture: &str) -> OwnerProbeResult {
    let output = match Command::new(helper)
        .arg(shortcut.modifiers.to_string())
        .arg(shortcut.virtual_key.to_string())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            return OwnerProbeResult::Error(format!("无法启动深度定位组件：{error}"));
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_helper_output(stdout.trim(), architecture)
}

fn executable_directory() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    Ok(executable
        .parent()
        .ok_or("KeyCrash 无法确定自身目录")?
        .into())
}

fn parse_helper_output(output: &str, architecture: &str) -> OwnerProbeResult {
    if output == "NOT_FOUND" {
        return OwnerProbeResult::NotFound;
    }

    if let Some(error) = output.strip_prefix("ERROR\t") {
        return OwnerProbeResult::Error(error.into());
    }

    let mut fields = output.splitn(5, '\t');
    if fields.next() != Some("FOUND") {
        return OwnerProbeResult::Error("深度定位组件返回了无法识别的结果".into());
    }

    let parsed = (|| {
        Some(OwnerProbeResult::Found {
            pid: fields.next()?.parse().ok()?,
            thread_id: fields.next()?.parse().ok()?,
            suppressed: fields.next()? == "1",
            path: fields.next().unwrap_or_default().into(),
            architecture: architecture.into(),
        })
    })();
    parsed.unwrap_or_else(|| OwnerProbeResult::Error("深度定位结果不完整".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_found_not_found_and_error_results() {
        assert_eq!(
            parse_helper_output("FOUND\t42\t7\t1\tC:\\Apps\\QQ.exe", "x64"),
            OwnerProbeResult::Found {
                pid: 42,
                thread_id: 7,
                path: "C:\\Apps\\QQ.exe".into(),
                suppressed: true,
                architecture: "x64".into(),
            }
        );
        assert_eq!(
            parse_helper_output("NOT_FOUND", "x86"),
            OwnerProbeResult::NotFound
        );
        assert_eq!(
            parse_helper_output("ERROR\tmissing hook", "x64"),
            OwnerProbeResult::Error("missing hook".into())
        );
    }
}
