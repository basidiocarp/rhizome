use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Result;
use tokio::net::UnixListener;
use serde_json::json;

use crate::tools::ToolDispatcher;

struct SocketServerGuard {
    sock_path: PathBuf,
    endpoint_path: PathBuf,
}

impl Drop for SocketServerGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.sock_path);
        let _ = std::fs::remove_file(&self.endpoint_path);
    }
}

pub async fn run_socket_server(project_root: PathBuf, unified: bool) -> Result<()> {
    let data_dir = spore::paths::data_dir("basidiocarp/rhizome");
    std::fs::create_dir_all(&data_dir)?;

    let sock_path = data_dir.join("rhizome.sock");
    let endpoint_path = data_dir.join("rhizome.endpoint.json");

    // Singleton guard: check if endpoint exists with a live PID
    if endpoint_path.exists()
        && let Ok(json_text) = std::fs::read_to_string(&endpoint_path)
        && let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_text)
        && let Some(pid_i64) = val["pid"].as_i64()
        && let Ok(pid) = i32::try_from(pid_i64)
    {
        let rc = unsafe { libc::kill(pid, 0) };
        let alive = rc == 0 || (rc == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM));
        if alive {
            eprintln!("rhizome socket server is already running (PID {pid})");
            return Ok(());
        }
    }

    // Clean up stale socket
    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path)?;

    // Write endpoint descriptor
    let endpoint_json = json!({
        "schema_version": "1.0",
        "transport": "unix-socket",
        "endpoint": sock_path.to_string_lossy(),
        "pid": std::process::id()
    });
    std::fs::write(&endpoint_path, serde_json::to_string_pretty(&endpoint_json)?)?;

    let _guard = SocketServerGuard {
        sock_path: sock_path.clone(),
        endpoint_path: endpoint_path.clone()
    };

    tracing::info!("rhizome socket server listening at {}", sock_path.display());

    let dispatcher = Arc::new(ToolDispatcher::new(project_root));

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::warn!("accept error (continuing): {e}");
                continue;
            }
        };
        let (reader, writer) = tokio::io::split(stream);
        let reader = tokio::io::BufReader::new(reader);
        let dispatcher_clone = Arc::clone(&dispatcher);

        tokio::spawn(async move {
            if let Err(e) = crate::server::run_mcp_over_stream(reader, writer, &dispatcher_clone, unified).await {
                tracing::warn!("socket connection ended: {e}");
            }
        });
    }
}
