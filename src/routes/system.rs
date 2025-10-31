use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::Ordering;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde_json::{json, Value};
use log::error;
use procfs::process::Process;
use procfs::page_size;
use fs2::available_space;

use crate::models::AppState;
use crate::config::save_settings_to_file;
use crate::utils::{dir_size, format_duration, format_bytes};

use procfs::prelude::*;

pub async fn get_status(
    app_data: web::Data<AppState>,
    _req: HttpRequest,
) -> impl Responder {
    app_data.request_counter.fetch_add(1, Ordering::Relaxed);

    let settings = app_data.settings.read().unwrap();
    let db_size_bytes = app_data.db.size_on_disk().unwrap_or(0);
    let index_path = Path::new(&settings.index_path);
    let index_size_bytes = dir_size(&index_path).unwrap_or(0);
    let disk_free_bytes = available_space(index_path).unwrap_or(0);

    let collections_lock = app_data.collections.read().unwrap();
    let total_collections = collections_lock.len();
    let mut total_items = 0usize;
    let mut total_index_segments = 0usize;
    for key in collections_lock.keys() {
        if let Ok(tree) = app_data.db.open_tree(key) {
            total_items += tree.len();
        }
        if let Some(info) = collections_lock.get(key) {
            let index = info.index.read().unwrap();
            if let Ok(reader) = index.reader() {
                total_index_segments += reader.searcher().segment_readers().len();
            }
        }
    }
    drop(collections_lock);

    let (resident_bytes, virtual_bytes) = match Process::myself() {
        Ok(proc) => match proc.statm() {
            Ok(statm) => {
                let page_size = page_size();
                (statm.resident as u64 * page_size, statm.size as u64 * page_size)
            }
            Err(e) => {
                error!("Failed to get memory stats: {}", e);
                (0, 0)
            }
        },
        Err(e) => {
            error!("Failed to get process info: {}", e);
            (0, 0)
        }
    };

    let uptime_seconds = app_data.start_time.elapsed().as_secs();
    let uptime_human = format_duration(uptime_seconds);
    let total_requests = app_data.request_counter.load(Ordering::Relaxed);

    let mut system_processes = Vec::new();
    if let Ok(current_proc) = procfs::process::Process::myself() {
        let name = match current_proc.stat() {
            Ok(stat) => stat.comm,
            Err(e) => {
                error!("Failed to get stat for current process: {}", e);
                "unknown".to_string()
            }
        };
        let resident_proc = match current_proc.statm() {
            Ok(statm) => statm.resident as u64 * page_size(),
            Err(e) => {
                error!("Failed to get statm for current process: {}", e);
                0
            }
        };
        let virtual_proc = match current_proc.statm() {
            Ok(statm) => statm.size as u64 * page_size(),
            Err(e) => {
                error!("Failed to get statm for current process: {}", e);
                0
            }
        };
        let cpu_time_ticks = match current_proc.stat() {
            Ok(stat) => stat.utime + stat.stime,
            Err(e) => {
                error!("Failed to get stat for current process: {}", e);
                0
            }
        };
        let status = match current_proc.status() {
            Ok(status) => status.state.to_string(),
            Err(e) => {
                error!("Failed to get status for current process: {}", e);
                "unknown".to_string()
            }
        };
        let threads = match current_proc.status() {
            Ok(status) => status.threads,
            Err(e) => {
                error!("Failed to get status for current process: {}", e);
                0
            }
        };

        system_processes.push(json!({
            "pid": current_proc.pid,
            "name": name,
            "resident_bytes": resident_proc,
            "resident_human": format_bytes(resident_proc),
            "virtual_bytes": virtual_proc,
            "virtual_human": format_bytes(virtual_proc),
            "cpu_time_ticks": cpu_time_ticks,
            "status": status,
            "threads": threads
        }));
    }

    let system_memory = match procfs::Meminfo::current() {
        Ok(mem) => {
            let total = mem.mem_total;
            let free = mem.mem_free;
            let available = mem.mem_available.unwrap_or(free);
            let used = total.saturating_sub(free);

            json!({
            "total_bytes": total,
            "total_human": format_bytes(total),
            "free_bytes": free,
            "free_human": format_bytes(free),
            "available_bytes": available,
            "available_human": format_bytes(available),
            "used_bytes": used,
            "used_human": format_bytes(used)
        })
        },
        Err(e) => {
            error!("Failed to get system memory: {}", e);
            json!({})
        }
    };

    let system_cpu = match procfs::KernelStats::current() {
        Ok(stat) => {
            let cpu_total = stat.total;
            json!({
                "total_user": cpu_total.user,
                "total_system": cpu_total.system,
                "total_idle": cpu_total.idle,
                "num_cores": stat.cpu_time.len()
            })
        }
        Err(e) => {
            error!("Failed to get system CPU: {}", e);
            json!({})
        }
    };

    let status_json = json!({
        "status": "healthy",
        "performance": "optimal",
        "memory": {
            "resident_bytes": resident_bytes,
            "resident_human": format_bytes(resident_bytes),
            "virtual_bytes": virtual_bytes,
            "virtual_human": format_bytes(virtual_bytes)
        },
        "db": {
            "size_bytes": db_size_bytes,
            "size_human": format_bytes(db_size_bytes)
        },
        "indexes": {
            "size_bytes": index_size_bytes,
            "size_human": format_bytes(index_size_bytes),
            "total_segments": total_index_segments
        },
        "disk": {
            "free_bytes": disk_free_bytes,
            "free_human": format_bytes(disk_free_bytes)
        },
        "total_collections": total_collections,
        "total_items": total_items,
        "uptime": {
            "seconds": uptime_seconds,
            "human": uptime_human
        },
        "requests": {
            "total": total_requests
        },
        "can_store_data": disk_free_bytes > 104_857_600,
        "system_processes": system_processes,
        "system_memory": system_memory,
        "system_cpu": system_cpu
    });
    HttpResponse::Ok().json(status_json)
}

pub async fn get_settings(
    app_data: web::Data<AppState>,
    _req: HttpRequest,
) -> impl Responder {
    let settings = app_data.settings.read().unwrap();
    let json_response = json!({
        "port": settings.port,
        "allowed_ips": settings.allowed_ips,
        "rate_limit_per_second": settings.rate_limit_per_second,
        "data_path": settings.data_path,
        "index_path": settings.index_path,
        "upsert_index_buffer": {
            "bytes": settings.upsert_index_buffer,
            "human": format_bytes(settings.upsert_index_buffer as u64)
        },
        "compact_index_buffer": {
            "bytes": settings.compact_index_buffer,
            "human": format_bytes(settings.compact_index_buffer as u64)
        },
        "default_scan_limit": settings.default_scan_limit,
        "max_scan_limit": settings.max_scan_limit,
        "payload_limit": {
            "bytes": settings.payload_limit,
            "human": format_bytes(settings.payload_limit as u64)
        }
    });
    HttpResponse::Ok().json(json_response)
}

pub async fn set_settings(
    app_data: web::Data<AppState>,
    _req: HttpRequest,
    json_data: web::Json<HashMap<String, Value>>,
) -> impl Responder {
    let updates = json_data.into_inner();
    let mut settings = app_data.settings.write().unwrap();
    let mut messages = Vec::new();

    for (key, value) in updates {
        match key.as_str() {
            k @ ("rate_limit_per_second" | "port" | "upsert_index_buffer" |
            "compact_index_buffer" | "default_scan_limit" | "max_scan_limit" |
            "payload_limit") => {
                if let Some(n) = value.as_u64() {
                    match k {
                        "rate_limit_per_second" => settings.rate_limit_per_second = n as u32,
                        "port" => settings.port = n as u16,
                        _ => {
                            let field = match k {
                                "upsert_index_buffer" => &mut settings.upsert_index_buffer,
                                "compact_index_buffer" => &mut settings.compact_index_buffer,
                                "default_scan_limit" => &mut settings.default_scan_limit,
                                "max_scan_limit" => &mut settings.max_scan_limit,
                                "payload_limit" => &mut settings.payload_limit,
                                _ => unreachable!(),
                            };
                            *field = n as usize;
                        }
                    }
                    messages.push(format!("Updated {}", k));
                }
            }
            "allowed_ips" => {
                if let Some(arr) = value.as_array() {
                    let ips: Vec<String> = arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                    settings.allowed_ips = ips;
                    messages.push("Updated allowed_ips".to_string());
                }
            }
            k @ ("data_path" | "index_path") => {
                if let Some(s) = value.as_str() {
                    match k {
                        "data_path" => settings.data_path = s.to_string(),
                        "index_path" => settings.index_path = s.to_string(),
                        _ => unreachable!(),
                    }
                    messages.push(format!("Updated {}", k));
                }
            }
            _ => messages.push(format!("Unknown setting: {}", key)),
        }
    }

    if let Err(e) = save_settings_to_file(&settings, "./settings.json") {
        error!("Failed to save settings: {}", e);
        return HttpResponse::InternalServerError().json(json!({"error": "Save failed"}));
    }
    HttpResponse::Ok().json(json!({"results": messages}))
}