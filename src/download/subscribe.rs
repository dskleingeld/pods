use super::Download;
use error_level::ErrorLevel;
use iced_futures::futures;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::io::{self, AsyncWriteExt};

#[derive(Debug, Clone)]
pub enum Progress {
    Started,
    Advanced(f32),
    Finished,
    Error(Error),
}

type Result<T> = std::result::Result<T, Error>;
#[derive(thiserror::Error, ErrorLevel, Debug, Clone)]
pub enum Error {
    #[report(warn)]
    #[error("Problem connecting to download")]
    Download(#[from] Arc<reqwest::Error>),
    #[report(error)]
    #[error("Could not store download")]
    Io(#[from] Arc<io::Error>),
    #[report(warn)]
    #[error("Do not know what file type this is (no extension given)")]
    NoExtension,
}

impl<H, I> iced_futures::subscription::Recipe<H, I> for Download
where
    H: std::hash::Hasher,
{
    type Output = Progress;

    fn hash(&self, state: &mut H) {
        use std::hash::Hash;
        std::any::TypeId::of::<Self>().hash(state);
        self.url.as_str().hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: futures::stream::BoxStream<'static, I>,
    ) -> futures::stream::BoxStream<'static, Self::Output> {
        Box::pin(futures::stream::unfold(
            State::Start(self.url, self.path),
            |state| async move { stream_state_machine(state).await },
        ))
    }
}

type StateResult = Option<(Progress, State)>;
async fn stream_state_machine(current: State) -> StateResult {
    match current {
        State::Start(url, path) => start(url, path)
            .await
            .unwrap_or_else(|e| Some((Progress::Error(e), State::Errored))),
        State::Downloading(data) => downloading(data)
            .await
            .unwrap_or_else(|e| Some((Progress::Error(e), State::Errored))),
        State::Finished(temp_path) => {
            let mut path = temp_path.clone(); // name.extension.part
            path.set_extension(""); // this removes the .part
            fs::rename(temp_path, path).await.unwrap();
            None
        }
        State::Errored => None,
    }
}

async fn start(url: reqwest::Url, path: PathBuf) -> Result<StateResult> {
    log::info!("downloading to file: {}", &path.to_string_lossy());
    let res = reqwest::get(url).await.map_err(Arc::from)?;
    let total = res.content_length();
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir).await.map_err(Arc::from)?;
    let file = fs::File::create(&path).await.map_err(Arc::from)?;
    let file = io::BufWriter::new(file);
    let state = DownloadData {
        res,
        file,
        total,
        downloaded: 0,
        path,
    };
    Ok(Some((Progress::Started, State::Downloading(state))))
}

async fn downloading(data: DownloadData) -> Result<StateResult> {
    let DownloadData {
        mut res,
        mut file,
        total,
        mut downloaded,
        path,
    } = data;
    match res.chunk().await.map_err(Arc::from)? {
        None => Ok(Some((Progress::Finished, State::Finished(path)))),
        Some(chunk) => {
            downloaded += chunk.len() as u64;
            file.write_all(&chunk).await.map_err(Arc::from)?;

            let percentage = total
                .map(|t| 100.0 * downloaded as f32 / t as f32)
                .unwrap_or(0.0);
            let progress = Progress::Advanced(percentage);
            let data = DownloadData {
                res,
                file,
                total,
                downloaded,
                path,
            };
            Ok(Some((progress, State::Downloading(data))))
        }
    }
}

#[derive(Debug)]
pub struct DownloadData {
    res: reqwest::Response,
    file: io::BufWriter<fs::File>,
    total: Option<u64>,
    downloaded: u64,
    path: PathBuf,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum State {
    Start(reqwest::Url, PathBuf),
    Downloading(DownloadData), //keep unboxed
    Finished(PathBuf),
    Errored,
}
