use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

pub async fn run_proxy() -> Result<()> {
    let data_dir = spore::paths::data_dir("basidiocarp/rhizome")?;
    let endpoint_path = data_dir.join("rhizome.endpoint.json");

    if !endpoint_path.exists() {
        anyhow::bail!(
            "rhizome socket server is not running.\n\
             Start it with: rhizome serve-socket --expanded\n\
             Or use stdio mode: rhizome serve --expanded"
        );
    }

    let json = std::fs::read_to_string(&endpoint_path)?;
    let endpoint_obj = serde_json::from_str::<serde_json::Value>(&json)
        .map_err(|e| anyhow::anyhow!("invalid endpoint descriptor: {e}"))?;

    let socket_path = endpoint_obj["endpoint"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing endpoint field in descriptor"))?;

    // Bridge: newline-delimited JSON stdin → unix socket → stdout
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    bridge_stdio_to_socket(stdin, stdout, socket_path).await
}

async fn bridge_stdio_to_socket(
    stdin: tokio::io::Stdin,
    stdout: tokio::io::Stdout,
    socket_path: &str,
) -> Result<()> {
    let stream = UnixStream::connect(socket_path).await
        .map_err(|e| anyhow::anyhow!("cannot connect to rhizome socket at {socket_path}: {e}\nIs rhizome serve-socket running?"))?;

    let (socket_reader, mut socket_writer) = tokio::io::split(stream);
    let mut socket_reader = BufReader::new(socket_reader);
    let mut stdin_reader = BufReader::new(stdin);
    let mut stdout = stdout;

    let mut stdin_line = String::new();
    let mut socket_line = String::new();

    loop {
        tokio::select! {
            result = stdin_reader.read_line(&mut stdin_line) => {
                let n = result?;
                if n == 0 { break; }
                socket_writer.write_all(stdin_line.as_bytes()).await?;
                socket_writer.flush().await?;
                stdin_line.clear();
            }
            result = socket_reader.read_line(&mut socket_line) => {
                let n = result?;
                if n == 0 { break; }
                stdout.write_all(socket_line.as_bytes()).await?;
                stdout.flush().await?;
                socket_line.clear();
            }
        }
    }
    Ok(())
}
