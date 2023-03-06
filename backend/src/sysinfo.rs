use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use axum::Json;
use http::StatusCode;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sysinfo::{CpuExt, DiskExt, System, SystemExt};
use tokio::task;

use crate::users::UserToken;

static SYSTEM_INFO: Lazy<Arc<Mutex<System>>> =
    Lazy::new(|| Arc::new(Mutex::new(System::new_all())));

#[derive(Serialize, Deserialize, Debug)]
pub struct SystemInfo {
    pub total_memory: u64,
    pub used_memory: u64,
    pub cpu_usage: f32,
    pub uptime: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: PathBuf,
    pub available_space: u64,
    pub total_space: u64,
}

pub async fn disk_info(path: PathBuf) -> Result<DiskInfo, &'static str> {
    let disksinfo: Result<Vec<DiskInfo>, &str> = task::spawn_blocking(|| {
        let mut sys = SYSTEM_INFO
            .lock()
            .map_err(|_| "could not lock system info")?;
        sys.refresh_disks();
        let disksinfo = sys
            .disks()
            .iter()
            .map(|disk| DiskInfo {
                name: disk.name().to_str().unwrap_or_default().to_owned(),
                mount_point: disk.mount_point().to_owned(),
                available_space: disk.available_space(),
                total_space: disk.total_space(),
            })
            .collect::<Vec<_>>();
        Ok(disksinfo)
    })
    .await
    .map_err(|_| "could not spawn system info task")?;
    let disksinfo = disksinfo?;
    // Work out which mount points are compatible with the path, and work out which is the more likely to host the given path
    corresponding_disk_info(disksinfo, path)
}

fn corresponding_disk_info(
    mut disksinfo: Vec<DiskInfo>,
    path: PathBuf,
) -> Result<DiskInfo, &'static str> {
    disksinfo.sort_by(|a, b| b.mount_point.partial_cmp(&a.mount_point).unwrap());
    disksinfo
        .into_iter()
        .find(|disk| {
            path.starts_with(&disk.mount_point)
                || if cfg!(windows) {
                    path.starts_with(format!(
                        "\\\\?\\{}",
                        disk.mount_point.to_str().unwrap_or("not-a-disk")
                    ))
                } else {
                    false
                }
        })
        .ok_or("no disks found")
}

pub async fn system_info(_user: UserToken) -> Result<Json<SystemInfo>, (StatusCode, &'static str)> {
    let sysinfo = task::spawn_blocking(|| {
        let mut sys = SYSTEM_INFO.lock().map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not lock system info",
            )
        })?;
        sys.refresh_system();
        Ok(SystemInfo {
            total_memory: sys.total_memory(),
            used_memory: sys.used_memory(),
            cpu_usage: sys.global_cpu_info().cpu_usage(),
            uptime: sys.uptime(),
        })
    })
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not spawn system info task",
        )
    })??;
    Ok(Json(sysinfo))
}

#[cfg(test)]
mod tests {
    use crate::sysinfo::{corresponding_disk_info, DiskInfo};
    use std::path::PathBuf;

    #[test]
    fn test_corresponding_disk_info() {
        let diskinfos = vec![
            DiskInfo {
                name: "disk_info_1".to_owned(),
                mount_point: PathBuf::from("/base/dir/"),
                available_space: 0,
                total_space: 0,
            },
            DiskInfo {
                name: "disk_info_2".to_owned(),
                mount_point: PathBuf::from("/base/"),
                available_space: 0,
                total_space: 0,
            },
            DiskInfo {
                name: "disk_info_3".to_owned(),
                mount_point: PathBuf::from("/base/dir/subdir/"),
                available_space: 0,
                total_space: 0,
            },
            DiskInfo {
                name: "disk_info_4".to_owned(),
                mount_point: PathBuf::from("/otherbase/dir/subdir/"),
                available_space: 0,
                total_space: 0,
            },
        ];
        let diskinfo =
            corresponding_disk_info(diskinfos, PathBuf::from("/base/dir/subdir/1")).unwrap();
        assert_eq!(diskinfo.mount_point, PathBuf::from("/base/dir/subdir/"));
    }
}
