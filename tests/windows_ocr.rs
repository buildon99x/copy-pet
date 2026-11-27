//! Exercise the shipped GUI-subsystem worker over its actual inherited pipes.
#![cfg(all(windows, not(feature = "portable")))]
use clipcat::clip_image::ClipImage;
use std::process::{Command, Stdio};
use tiny_skia::{Color, Pixmap, Transform};

#[test]
fn installed_english_recognizer_reads_synthetic_image() {
    let mut pixels = Pixmap::new(480, 100).unwrap();
    pixels.fill(Color::WHITE);
    clipcat::sysfont::draw(
        &mut pixels,
        "Receipt 12345",
        20.0,
        20.0,
        5.5,
        (0, 0, 0, 255),
        Transform::identity(),
    );
    let image = ClipImage::from_pixmap(&pixels).unwrap();
    let output = run_worker(&image);
    assert!(
        output.status.success(),
        "Windows OCR exit {:?}",
        output.status.code()
    );
    let text = String::from_utf8(output.stdout).unwrap().to_lowercase();
    assert!(
        text.contains("receipt") && text.contains("12345"),
        "{text:?}"
    );
}

fn run_worker(image: &ClipImage) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_clipcat"))
        .arg("--ocr-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    serde_json::to_writer(child.stdin.take().unwrap(), &image).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("OCR worker exceeded its deadline");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    child.wait_with_output().unwrap()
}

#[test]
fn worker_distinguishes_empty_image_from_invalid_pixels() {
    let mut pixels = Pixmap::new(320, 100).unwrap();
    pixels.fill(Color::WHITE);
    let mut image = ClipImage::from_pixmap(&pixels).unwrap();
    assert_eq!(run_worker(&image).status.code(), Some(1));
    image.width = 0;
    assert_eq!(run_worker(&image).status.code(), Some(3));
}
