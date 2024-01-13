extern crate ffmpeg_next as ffmpeg;

use ffmpeg::format::{input, Pixel};
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context, flag::Flags};
use ffmpeg::util::frame::video::Video;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::time::Instant;

use image::io::Reader as ImageReader;
use image::{imageops, DynamicImage, ImageBuffer, ImageError, ImageFormat, RgbImage};

fn main() -> Result<(), ffmpeg::Error> {
    ffmpeg::init().unwrap();

    let chunk_duration_sec = 2;
    let path = env::args()
        .nth(1)
        .unwrap_or("examples/example_video.mp4".to_string());
    if let Ok(mut ictx) = input(&path) {
        let input = ictx
            .streams()
            .best(Type::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)?;
        let video_stream_index = input.index();

        let context_decoder = ffmpeg::codec::context::Context::from_parameters(input.parameters())?;
        let mut decoder = context_decoder.decoder().video()?;

        let original_width = decoder.width();
        let original_height = decoder.height();

        // Fixed height at 720 pixels
        let dest_height = 720 as u32;

        // Calculate the destination width to maintain the aspect ratio
        let aspect_ratio = original_width as f32 / original_height as f32;
        let dest_width = (dest_height as f32 * aspect_ratio).round() as u32;

        let mut scaler = Context::get(
            decoder.format(),
            decoder.width(),
            decoder.height(),
            Pixel::RGB24,
            dest_width,
            dest_height,
            Flags::BILINEAR,
        )?;

        let mut frame_index = 0;

        let frame_rate = input.avg_frame_rate();

        let frames_per_chunk =
            frame_rate.numerator() as i64 * chunk_duration_sec / frame_rate.denominator() as i64;

        let mut receive_and_process_decoded_frames =
            |decoder: &mut ffmpeg::decoder::Video| -> Result<(), ffmpeg::Error> {
                let mut decoded = Video::empty();
                while decoder.receive_frame(&mut decoded).is_ok() {
                    let mut rgb_frame = Video::empty();
                    scaler.run(&decoded, &mut rgb_frame)?;
                    edge_detect(&rgb_frame, frame_index).unwrap();
                    //apply_blur(&rgb_frame, frame_index).unwrap();
                    frame_index += 1;
                }
                Ok(())
            };

        for (stream, packet) in ictx.packets() {
            if stream.index() == video_stream_index {
                decoder.send_packet(&packet)?;
                receive_and_process_decoded_frames(&mut decoder)?;
            }
        }
        decoder.send_eof()?;
        receive_and_process_decoded_frames(&mut decoder)?;
    }

    Ok(())
}

fn save_file(frame: &Video, index: usize) -> std::result::Result<(), std::io::Error> {
    let mut file = File::create(format!("frame{}.ppm", index))?;
    file.write_all(format!("P6\n{} {}\n255\n", frame.width(), frame.height()).as_bytes())?;
    file.write_all(frame.data(0))?;
    Ok(())
}

fn apply_blur(frame: &Video, index: usize) -> std::result::Result<(), Box<dyn Error>> {
    println!("Processing frame {}", index);
    let start_total = Instant::now();

    let start = Instant::now();
    let width = frame.width() as u32;
    let height = frame.height() as u32;
    let frame_data = frame.data(0);

    // Create an RgbImage from the raw frame data
    let rgb_image: RgbImage = ImageBuffer::from_raw(width, height, frame_data.to_vec())
        .ok_or("Failed to create image from raw data")?;
    let end = Instant::now();
    println!(
        "Time to create source image: {:?}",
        end.duration_since(start)
    );

    let start = Instant::now();
    // Apply a Gaussian blur to the image
    let blurred_image = imageops::blur(&rgb_image, 5.0); // Adjust sigma as needed
    let end = Instant::now();
    println!("Time to apply blur: {:?}", end.duration_since(start));

    let start = Instant::now();
    // Save the blurred image to a file
    blurred_image.save(format!("frames/blurred_frame{}.png", index))?;
    let end = Instant::now();
    println!(
        "Time to encode and write file: {:?}",
        end.duration_since(start)
    );

    let end_total = Instant::now();
    println!(
        "Total time for apply_blur: {:?}\n",
        end_total.duration_since(start_total)
    );

    Ok(())
}
fn edge_detect(frame: &Video, index: usize) -> Result<(), Box<dyn Error>> {
    println!("Processing frame {}", index);
    let start_total = Instant::now();

    let start = Instant::now();
    let width = frame.width() as u32;
    let height = frame.height() as u32;
    let frame_data = frame.data(0);

    // Create an RgbImage from the raw frame data
    let source_image: RgbImage = ImageBuffer::from_raw(width, height, frame_data.to_vec())
        .ok_or("Failed to create image from raw data")?;
    let end = Instant::now();
    println!(
        "Time to create source image: {:?}",
        end.duration_since(start)
    );

    let start = Instant::now();
    // Convert to grayscale and apply edge detection
    let gray_image = DynamicImage::ImageRgb8(source_image).into_luma8();
    let detect = edge_detection::canny(
        gray_image, 1.2,  // sigma
        0.2,  // strong threshold
        0.01, // weak threshold
    );
    let end = Instant::now();
    println!(
        "Time to apply edge detection: {:?}",
        end.duration_since(start)
    );

    let start = Instant::now();
    // Save the resulting image
    let mut file = File::create(format!("frames/frame{}.png", index))?;
    detect.as_image().write_to(&mut file, ImageFormat::Png)?;
    let end = Instant::now();
    println!(
        "Time to encode and write file: {:?}",
        end.duration_since(start)
    );

    let end_total = Instant::now();
    println!(
        "Total time for edge_detect: {:?}\n",
        end_total.duration_since(start_total)
    );

    Ok(())
}
