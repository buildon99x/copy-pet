//! A single isolated recognizer uses installed Windows languages. Pipe IO runs off
//! the UI thread. A deadline or closing the app kills and reaps the child.
use crate::clip_image::{ClipImage, MAX_IMAGE_BYTES};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::time::{Duration, Instant};
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Security::Cryptography::CryptographicBuffer;
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};

use crate::clipboard::MAX_OCR_TEXT as TEXT_LIMIT;
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Failure {
    Empty,
    Unavailable,
    Failed,
}
type Outcome = Result<String, Failure>;

fn recognize(image: ClipImage) -> Outcome {
    let engine =
        OcrEngine::TryCreateFromUserProfileLanguages().map_err(|_| Failure::Unavailable)?;
    let max = OcrEngine::MaxImageDimension().map_err(|_| Failure::Unavailable)?;
    if image.width.max(image.height) > max {
        return Err(Failure::Failed);
    }
    let pixels = image.pixmap().ok_or(Failure::Failed)?;
    let mut data = pixels.take();
    for pixel in data.chunks_exact_mut(4) {
        // Composite premultiplied transparent pixels onto white for OCR.
        let background = 255 - pixel[3];
        pixel.swap(0, 2);
        for value in &mut pixel[..3] {
            *value = value.saturating_add(background);
        }
        pixel[3] = 255;
    }
    let buffer = CryptographicBuffer::CreateFromByteArray(&data).map_err(|_| Failure::Failed)?;
    let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        image.width as i32,
        image.height as i32,
    )
    .map_err(|_| Failure::Failed)?;
    let result = engine
        .RecognizeAsync(&bitmap)
        .and_then(|op| op.get())
        .map_err(|_| Failure::Failed)?;
    let text = result.Text().map_err(|_| Failure::Failed)?.to_string();
    if text.trim().is_empty() {
        Err(Failure::Empty)
    } else if text.len() > TEXT_LIMIT {
        Err(Failure::Failed)
    } else {
        Ok(text)
    }
}

pub fn worker() -> i32 {
    // The parent normally enforces its shorter deadline. This also terminates
    // the isolated worker if the app is forcibly closed during a WinRT call.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(25));
        std::process::exit(3);
    });
    let limit = MAX_IMAGE_BYTES.div_ceil(3) * 4 + 256;
    let mut input = Vec::new();
    if std::io::stdin()
        .take(limit as u64 + 1)
        .read_to_end(&mut input)
        .is_err()
        || input.len() > limit
    {
        return 3;
    }
    let Ok(image) = serde_json::from_slice(&input) else {
        return 3;
    };
    // This dedicated worker thread owns WinRT initialization and all projections.
    if unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_err() {
        return 2;
    }
    let outcome = recognize(image);
    unsafe { RoUninitialize() };
    match outcome {
        Ok(text) if std::io::stdout().lock().write_all(text.as_bytes()).is_ok() => 0,
        Err(Failure::Empty) => 1,
        Err(Failure::Unavailable) => 2,
        _ => 3,
    }
}

fn run(image: &ClipImage, cancel: &AtomicBool) -> Outcome {
    let exe = std::env::current_exe().map_err(|_| Failure::Failed)?;
    let mut child = Command::new(exe)
        .arg("--ocr-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| Failure::Failed)?;
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    std::thread::scope(|scope| {
        let writer = scope.spawn(move || serde_json::to_writer(stdin, image));
        let reader = scope.spawn(move || {
            let mut bytes = Vec::new();
            stdout.take(TEXT_LIMIT as u64 + 1).read_to_end(&mut bytes)?;
            Ok::<_, std::io::Error>(bytes)
        });
        let start = Instant::now();
        let code = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status.code(),
                Ok(None)
                    if !cancel.load(Ordering::Relaxed)
                        && start.elapsed() < Duration::from_secs(20) =>
                {
                    std::thread::sleep(Duration::from_millis(50))
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
            }
        };
        writer
            .join()
            .map_err(|_| Failure::Failed)?
            .map_err(|_| Failure::Failed)?;
        let bytes = reader
            .join()
            .map_err(|_| Failure::Failed)?
            .map_err(|_| Failure::Failed)?;
        match code {
            Some(0) if !bytes.is_empty() && bytes.len() <= TEXT_LIMIT => {
                String::from_utf8(bytes).map_err(|_| Failure::Failed)
            }
            Some(1) => Err(Failure::Empty),
            Some(2) => Err(Failure::Unavailable),
            _ => Err(Failure::Failed),
        }
    })
}

pub(crate) struct Job {
    pub id: u64,
    pub generation: u64,
    pub epoch: u64,
    pub result: mpsc::Receiver<(ClipImage, Outcome)>,
    cancel: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Job {
    pub fn start(id: u64, generation: u64, epoch: u64, image: ClipImage) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let (tx, result) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            let outcome = run(&image, &flag);
            let _ = tx.send((image, outcome));
        });
        Self {
            id,
            generation,
            epoch,
            result,
            cancel,
            thread: Some(thread),
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
