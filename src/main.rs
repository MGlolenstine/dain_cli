use log::{debug, error, info};
use std::io::Write;
//TODO: Implement audio support
//TODO: Figure out what's up with the subtitles and multiple channels

fn main() {
    let mut args = std::env::args();
    pretty_env_logger::init();
    if args.len() < 4 {
        eprintln!(
            "Wrong number of arguments!\n{} <input_video> <target_framerate> <output_video>",
            args.next().unwrap()
        );
        return;
    }
    if !std::path::Path::new("dain").exists()
        || std::path::Path::new("dain").read_dir().unwrap().count() == 0
    {
        info!("Dain doesn't seem to be present. Installing...");
        install_dain();
    }
    let _program_call = args.next().unwrap();
    let input_video = args.next().unwrap();
    let target_framerate = args.next().unwrap().parse::<f32>().unwrap();
    let output_video = args.next().unwrap();
    info!("Getting framerate");
    let fps = get_framerate(&input_video).unwrap();
    info!("Framerate is {}", fps);
    info!("Turning video into frames");
    let original_frame_count = video_into_frames(&input_video).unwrap();
    info!("Original frame count is: {}", original_frame_count);
    let new_frame_count = calculate_frame_count(fps, original_frame_count, target_framerate);
    info!("New frame count is: {}", new_frame_count);
    info!("Running DAIN interpolator (This might take a bit)...");
    if let Err(e) = dain_process_frames(new_frame_count) {
        error!("{:#?}", e);
        #[cfg(target_os = "windows")]
        error!("Something went wrong while running DAIN!\nRun `dain/dain-ncnn-vulkan.exe -i original_frames -o out_frames` by hand to see the error!");
        #[cfg(not(target_os = "windows"))]
        error!("Something went wrong while running DAIN!\nRun `./dain/dain-ncnn-vulkan -i original_frames -o out_frames` by hand to see the error!");
        return;
    }
    info!("DAIN interpolator completed");
    info!("Putting video back together...");
    if let Err(e) = frames_into_video(&output_video, target_framerate) {
        error!("{:#?}", e);
    }
    info!("Conversion completed successfully! Enjoy!");
}

fn calculate_frame_count(fps: f32, framecount: usize, target_framerate: f32) -> u32 {
    let framecount = framecount as f32;
    ((framecount / fps) * target_framerate).round() as u32
}

fn get_framerate(path: &str) -> Result<f32, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("ffprobe")
        .arg(path)
        .output()?
        .stderr;
    let data = String::from_utf8_lossy(&output);
    let r = regex::RegexBuilder::new(r#"([\d]+.[\d]+|[\d]+) fps"#).build()?;
    for l in data.lines().filter(|a| a.contains(" fps,")) {
        if let Some(a) = r.captures(l) {
            let fps = a.get(1).unwrap();
            let fps = fps.as_str().parse::<f32>()?;
            return Ok(fps);
        } else {
            eprintln!("the following line doesn't contain fps: {:#?}", l);
        }
    }
    Ok(0.0)
}

fn video_into_frames(path: &str) -> Result<usize, Box<dyn std::error::Error>> {
    if !std::path::Path::new("original_frames").exists() {
        std::fs::create_dir("original_frames")?;
    }
    let output = std::process::Command::new("ffmpeg")
        .args(&["-i", path, "original_frames/%08d.png"])
        .output()?
        .stderr;
    let data = String::from_utf8_lossy(&output);
    debug!("FFMPEG Video -> Frames stderr:\n{:#?}", data);
    Ok(std::fs::read_dir("original_frames")?.count())
}

fn frames_into_video(path: &str, target_framerate: f32) -> Result<(), Box<dyn std::error::Error>> {
    let output = std::process::Command::new("ffmpeg")
        .args(&[
            "-r",
            &format!("{}", target_framerate),
            "-i",
            "out_frames/%08d.png",
            "-c:v",
            "libx264",
            path,
        ])
        .output()?
        .stderr;
    let data = String::from_utf8_lossy(&output);
    debug!("FFMPEG Frames -> Video stderr:\n{:#?}", data);
    Ok(())
}

fn dain_process_frames(new_frame_count: u32) -> Result<(), Box<dyn std::error::Error>> {
    if !std::path::Path::new("out_frames").exists() {
        std::fs::create_dir("out_frames")?;
    }
    #[cfg(not(target_os = "windows"))]
    let command = "./dain/dain-ncnn-vulkan";
    #[cfg(target_os = "windows")]
    let command = "./dain/dain-ncnn-vulkan.exe";
    let output = std::process::Command::new(command)
        .args(&[
            "-i",
            "original_frames",
            "-o",
            "out_frames",
            "-n",
            &format!("{}", new_frame_count),
        ])
        .output()?
        .stderr;
    let data = String::from_utf8_lossy(&output);
    debug!("DAIN Frames -> Frames stderr:\n{:#?}", data);
    Ok(())
}

fn install_dain() {
    #[cfg(target_os = "windows")]
        let url = "https://github.com/nihui/dain-ncnn-vulkan/releases/download/20210210/dain-ncnn-vulkan-20210210-windows.zip";
    #[cfg(target_os = "windows")]
    let filename = "dain-ncnn-vulkan-20210210-windows";
    #[cfg(target_os = "linux")]
        let url = "https://github.com/nihui/dain-ncnn-vulkan/releases/download/20210210/dain-ncnn-vulkan-20210210-ubuntu.zip";
    #[cfg(target_os = "linux")]
    let filename = "dain-ncnn-vulkan-20200210-ubuntu";
    #[cfg(target_os = "macos")]
        let url = "https://github.com/nihui/dain-ncnn-vulkan/releases/download/20210210/dain-ncnn-vulkan-20210210-macos.zip";
    #[cfg(target_os = "macos")]
    let filename = "dain-ncnn-vulkan-20210210-macos";

    info!("Downloading DAIN...");
    let dain = reqwest::blocking::get(url)
        .unwrap()
        .bytes()
        .unwrap()
        .to_vec();
    info!("Writing dain.zip...");
    // let mut pos = 0;
    let mut buffer = std::fs::File::create("dain.zip").unwrap();
    buffer.write_all(&dain[..]).unwrap();
    buffer.flush().unwrap();
    let buffer = std::fs::OpenOptions::new()
        .read(true)
        .open("dain.zip")
        .unwrap();
    info!("dain.zip downloaded. Extracting...");
    let mut zip = zip::read::ZipArchive::new(&buffer).unwrap();
    zip.extract(".").unwrap();
    std::fs::rename(filename, "dain").unwrap();
}
