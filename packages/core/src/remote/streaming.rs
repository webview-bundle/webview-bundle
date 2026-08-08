use crate::remote::RemoteOnDownload;
use crate::util::cancellation::Cancellation;
use futures_util::StreamExt;
use std::path::Path;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

pub async fn stream_to_file(
  resp: reqwest::Response,
  filepath: &Path,
  cancel: Option<Cancellation>,
  on_download: Option<Arc<RemoteOnDownload>>,
) -> crate::Result<()> {
  if let Some(dir) = filepath.parent() {
    tokio::fs::create_dir_all(dir).await?;
  }

  let total_size = resp.content_length();
  let mut file = tokio::fs::File::create(filepath).await?;
  let mut stream = resp.bytes_stream();
  let mut downloaded_bytes: u64 = 0;
  let cancel = cancel.unwrap_or_default();
  loop {
    let chunk = tokio::select! {
      // Keep poll order top-to-down so always check the cancellation first.
      biased;
      _ = cancel.cancelled() => return Err(crate::Error::Cancelled),
      chunk_result = stream.next() => match chunk_result {
        Some(ret) => ret?,
        None => break,
      },
    };

    file.write_all(&chunk).await?;
    downloaded_bytes += chunk.len() as u64;

    if let Some(ref on_download) = on_download {
      on_download(downloaded_bytes, total_size, "".to_string());
    }
  }

  file.flush().await?;
  file.sync_all().await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::ErrorCode;
  use crate::testing::TempDir;
  use std::sync::Mutex;

  type Recorded = Arc<Mutex<Vec<(u64, Option<u64>)>>>;

  fn bytes_response(data: Vec<u8>) -> reqwest::Response {
    reqwest::Response::from(http::Response::new(reqwest::Body::from(data)))
  }

  fn stream_response(chunks: Vec<Result<Vec<u8>, std::io::Error>>) -> reqwest::Response {
    reqwest::Response::from(http::Response::new(reqwest::Body::wrap_stream(
      futures_util::stream::iter(chunks),
    )))
  }

  fn recorder() -> (Recorded, Arc<RemoteOnDownload>) {
    let recorded: Recorded = Arc::new(Mutex::new(vec![]));
    let sink = recorded.clone();
    let on_download: Arc<RemoteOnDownload> = Arc::new(move |downloaded, total, _| {
      sink.lock().unwrap().push((downloaded, total));
    });
    (recorded, on_download)
  }

  fn entries(dir: &Path) -> Vec<String> {
    let mut entries = std::fs::read_dir(dir)
      .unwrap()
      .map(|x| x.unwrap().file_name().to_string_lossy().to_string())
      .collect::<Vec<_>>();
    entries.sort();
    entries
  }

  #[tokio::test]
  async fn writing() {
    let temp = TempDir::new();
    let filepath = temp.dir().join("app.wvb");

    stream_to_file(bytes_response(b"hello wvb".to_vec()), &filepath, None, None)
      .await
      .unwrap();

    assert_eq!(tokio::fs::read(&filepath).await.unwrap(), b"hello wvb");
    assert_eq!(entries(temp.dir()), vec!["app.wvb"]);
  }

  #[tokio::test]
  async fn writing_empty() {
    let temp = TempDir::new();
    let filepath = temp.dir().join("app.wvb");

    stream_to_file(bytes_response(vec![]), &filepath, None, None)
      .await
      .unwrap();

    assert!(tokio::fs::read(&filepath).await.unwrap().is_empty());
    assert_eq!(entries(temp.dir()), vec!["app.wvb"]);
  }

  #[tokio::test]
  async fn writing_chunks() {
    let temp = TempDir::new();
    let filepath = temp.dir().join("app.wvb");
    let response = stream_response(vec![
      Ok(b"first".to_vec()),
      Ok(b"second".to_vec()),
      Ok(b"third".to_vec()),
    ]);

    stream_to_file(response, &filepath, None, None)
      .await
      .unwrap();

    assert_eq!(
      tokio::fs::read(&filepath).await.unwrap(),
      b"firstsecondthird"
    );
  }

  #[tokio::test]
  async fn writing_over_existing_file() {
    let temp = TempDir::new();
    let filepath = temp.dir().join("app.wvb");
    tokio::fs::write(&filepath, b"old").await.unwrap();

    stream_to_file(bytes_response(b"new".to_vec()), &filepath, None, None)
      .await
      .unwrap();

    assert_eq!(tokio::fs::read(&filepath).await.unwrap(), b"new");
    assert_eq!(entries(temp.dir()), vec!["app.wvb"]);
  }

  #[tokio::test]
  async fn writing_concurrently() {
    let temp = TempDir::new();
    let first = temp.dir().join("first.wvb");
    let second = temp.dir().join("second.wvb");

    let (a, b) = tokio::join!(
      stream_to_file(bytes_response(vec![b'a'; 512]), &first, None, None),
      stream_to_file(bytes_response(vec![b'b'; 512]), &second, None, None),
    );
    a.unwrap();
    b.unwrap();

    assert_eq!(tokio::fs::read(&first).await.unwrap(), vec![b'a'; 512]);
    assert_eq!(tokio::fs::read(&second).await.unwrap(), vec![b'b'; 512]);
    assert_eq!(entries(temp.dir()), vec!["first.wvb", "second.wvb"]);
  }

  #[tokio::test]
  async fn on_download_callback() {
    let temp = TempDir::new();
    let filepath = temp.dir().join("app.wvb");
    let (recorded, on_download) = recorder();

    stream_to_file(
      bytes_response(vec![b'x'; 1024]),
      &filepath,
      None,
      Some(on_download),
    )
    .await
    .unwrap();

    let recorded = recorded.lock().unwrap().clone();
    assert!(!recorded.is_empty());
    assert!(recorded.iter().all(|(_, total)| *total == Some(1024)));
    assert!(
      recorded
        .iter()
        .map(|(downloaded, _)| *downloaded)
        .is_sorted()
    );
    assert_eq!(recorded.last().unwrap().0, 1024);
  }

  #[tokio::test]
  async fn on_download_callback_no_content_length() {
    let temp = TempDir::new();
    let filepath = temp.dir().join("app.wvb");
    let (recorded, on_download) = recorder();
    let response = stream_response(vec![Ok(vec![b'a'; 4]), Ok(vec![b'b'; 6])]);

    stream_to_file(response, &filepath, None, Some(on_download))
      .await
      .unwrap();

    assert_eq!(*recorded.lock().unwrap(), vec![(4, None), (10, None)]);
  }

  #[tokio::test]
  async fn cancelled_already() {
    let temp = TempDir::new();
    let filepath = temp.dir().join("app.wvb");
    let cancellation = Cancellation::new();
    cancellation.cancel();

    let err = stream_to_file(
      bytes_response(vec![b'x'; 64]),
      &filepath,
      Some(cancellation),
      None,
    )
    .await
    .unwrap_err();

    assert_eq!(err.code(), ErrorCode::Cancelled);
    assert!(entries(temp.dir()).is_empty());
  }

  #[tokio::test]
  async fn cancel() {
    let temp = TempDir::new();
    let filepath = temp.dir().join("app.wvb");
    let cancellation = Cancellation::new();
    let cancellation_cloned = cancellation.clone();
    let on_download: Arc<RemoteOnDownload> = Arc::new(move |_, _, _| cancellation_cloned.cancel());
    let response = stream_response(vec![Ok(vec![b'a'; 4]), Ok(vec![b'b'; 4])]);

    let err = stream_to_file(response, &filepath, None, Some(on_download))
      .await
      .unwrap_err();

    assert_eq!(err.code(), ErrorCode::Cancelled);
    assert!(entries(temp.dir()).is_empty());
  }

  #[tokio::test]
  async fn propagates_error() {
    let temp = TempDir::new();
    let filepath = temp.dir().join("app.wvb");
    let response = stream_response(vec![
      Ok(vec![b'a'; 4]),
      Err(std::io::Error::other("connection reset")),
    ]);

    let err = stream_to_file(response, &filepath, None, None)
      .await
      .unwrap_err();

    assert_eq!(err.code(), ErrorCode::HttpClient);
    assert!(entries(temp.dir()).is_empty());
  }

  #[tokio::test]
  async fn create_dir_all() {
    let temp = TempDir::new();
    let dir = temp.dir().join("missing");
    let filepath = dir.join("app.wvb");

    stream_to_file(bytes_response(b"hello".to_vec()), &filepath, None, None)
      .await
      .unwrap();

    assert_eq!(tokio::fs::read(&filepath).await.unwrap(), b"hello");
    assert_eq!(entries(&dir), vec!["app.wvb"]);
  }
}
