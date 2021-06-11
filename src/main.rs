use linya::{Bar, Progress};
use log::{debug, error, info};
use pretty_env_logger::env_logger::Builder;
use pretty_env_logger::env_logger::Env;
use std::io::Write;
use std::time::{Duration, Instant};
use tokio::select;
//TODO: Implement audio support
//TODO: Figure out what's up with the subtitles and multiple channels
//TODO: Add option for chosing RIFE model

#[tokio::main]
async fn main() {
    let mut args = std::env::args();
    // pretty_env_logger::init();
    Builder::from_env(Env::default().default_filter_or("debug")).init();
    if args.len() < 4 {
        error!(
            "Wrong number of arguments!\n{} <input_video> <output_video> <framework> [<target_framerate>]\nframework can be either `rife` or `dain`\nRife: Fast framework, but it can only double the framerate\nDAIN: Very slow model, but it can set custom framerate\ntarget_framerate: Only respected in DAIN, RIFE only does 2x on current framerate.\nIf not specified for DAIN, it defaults to 60.0",
            args.next().unwrap()
        );
        return;
    }
    let _program_call = args.next().unwrap();
    let input_video = args.next().unwrap();
    let output_video = args.next().unwrap();
    let framework = args.next().unwrap();
    let target_framerate = args
        .next()
        .unwrap_or("60.0".to_owned())
        .parse::<f32>()
        .unwrap();
    let time = Instant::now();
    match framework.as_str() {
        "dain" => {
            if !std::path::Path::new("dain").exists()
                || std::path::Path::new("dain").read_dir().unwrap().count() == 0
            {
                info!("Dain doesn't seem to be present. Installing...");
                install_dain().await;
            }
        }
        "rife" => {
            if !std::path::Path::new("rife").exists()
                || std::path::Path::new("rife").read_dir().unwrap().count() == 0
            {
                info!("Rife doesn't seem to be present. Installing...");
                install_rife().await;
            }
        }
        _ => {}
    }
    info!("Getting framerate");
    let fps = get_framerate(&input_video).unwrap();
    info!("Framerate is {}", fps);
    info!("Turning video into frames");
    let original_frame_count = video_into_frames(&input_video).await.unwrap();
    info!("Original frame count is: {}", original_frame_count);
    let new_frame_count =
        calculate_frame_count(fps, original_frame_count, target_framerate, &framework);
    info!("New frame count is: {}", new_frame_count);
    match framework.as_str() {
        "dain" => {
            info!("Running DAIN interpolator (This might take a bit)...");
            let (sender, mut receiver) = tokio::sync::mpsc::channel(8);
            tokio::spawn(tokio::task::spawn_blocking(move || {
                if let Err(e) = dain_process_frames(new_frame_count) {
                    error!("{:#?}", e);
                    #[cfg(target_os = "windows")]
                    error!("Something went wrong while running DAIN!\nRun `dain/dain-ncnn-vulkan.exe -i original_frames -o out_frames` by hand to see the error!");
                    #[cfg(not(target_os = "windows"))]
                    error!("Something went wrong while running DAIN!\nRun `./dain/dain-ncnn-vulkan -i original_frames -o out_frames` by hand to see the error!");
                    tokio::spawn(async move { sender.send(false).await });
                    return;
                }
                tokio::spawn(async move { sender.send(true).await });
            }));
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            let mut progress = Progress::new();
            let bar: Bar = progress.bar(new_frame_count as usize, "DAIN RENDER PROGRESS");
            loop {
                select! {
                    _ = interval.tick() => {
                        if let Ok(a) = std::fs::read_dir("out_frames"){
                            let amount = a.count();
                            progress.set_and_draw(&bar, amount);
                        }
                        // println!("Updating the progress bar. {}", progress_bar);
                    },
                    Some(_) = receiver.recv() => {
                        break;
                    }
                }
            }
            info!("DAIN interpolator completed");
        }
        "rife" => {
            info!("Running RIFE interpolator (This might take a bit)...");
            let (sender, mut receiver) = tokio::sync::mpsc::channel(8);
            tokio::spawn(tokio::task::spawn_blocking(move || {
                if let Err(e) = rife_process_frames(/*new_frame_count*/) {
                    error!("{:#?}", e);
                    #[cfg(target_os = "windows")]
                    error!("Something went wrong while running RIFE!\nRun `./rife/rife-ncnn-vulkan.exe -i original_frames -o out_frames` by hand to see the error!");
                    #[cfg(not(target_os = "windows"))]
                    error!("Something went wrong while running RIFE!\nRun `./rife/rife-ncnn-vulkan -i original_frames -o out_frames` by hand to see the error!");
                    tokio::spawn(async move { sender.send(false).await });
                    return;
                }
                tokio::spawn(async move { sender.send(true).await });
            }));
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            let mut progress = Progress::new();
            let bar: Bar = progress.bar(new_frame_count as usize, "RIFE RENDER PROGRESS");
            loop {
                select! {
                    _ = interval.tick() => {
                        if let Ok(a) = std::fs::read_dir("out_frames"){
                            let amount = a.count();
                            progress.set_and_draw(&bar, amount);
                        }
                        // println!("Updating the progress bar. {}", progress_bar);
                    },
                    Some(_) = receiver.recv() => {
                        break;
                    }
                }
            }
            info!("RIFE interpolator completed");
        }
        _ => {
            error!("That framework doesn't exist!");
            return;
        }
    }
    info!("Putting video back together...");
    let target_framerate = match framework.as_str() {
        "dain" => target_framerate,
        "rife" => fps * 2.0,
        _ => 0.0,
    };
    if let Err(e) = frames_into_video(&output_video, target_framerate) {
        error!("{:#?}", e);
    }
    cleanup();
    info!(
        "Conversion completed successfully in {}s! Enjoy!",
        time.elapsed().as_secs()
    );
}

fn cleanup() {
    std::fs::remove_dir_all("original_frames").unwrap();
    std::fs::remove_dir_all("out_frames").unwrap();
}

fn calculate_frame_count(
    fps: f32,
    framecount: usize,
    target_framerate: f32,
    framework: &str,
) -> u32 {
    match framework {
        "dain" => {
            let framecount = framecount as f32;
            ((framecount / fps) * target_framerate).round() as u32
        }
        "rife" => (framecount as f32 * 2.0) as u32,
        _ => 0u32,
    }
}

fn get_framerate(path: &str) -> Result<f32, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("ffprobe")
        .arg(path)
        .output()
        .expect("FFMPEG probably not installed! ffprobe command missing.")
        .stderr;
    let data = String::from_utf8_lossy(&output);
    let r = regex::RegexBuilder::new(r#"([\d]+.[\d]+|[\d]+) fps"#).build()?;
    for l in data.lines().filter(|a| a.contains(" fps,")) {
        if let Some(a) = r.captures(l) {
            let fps = a.get(1).unwrap();
            let fps = fps.as_str().parse::<f32>()?;
            return Ok(fps);
        } else {
            error!("the following line doesn't contain fps: {:#?}", l);
        }
    }
    Ok(0.0)
}

async fn video_into_frames(path: &str) -> Result<usize, Box<dyn std::error::Error>> {
    if !std::path::Path::new("original_frames").exists() {
        std::fs::create_dir("original_frames")?;
    } else {
        std::fs::remove_dir_all("original_frames")?;
        std::fs::create_dir("original_frames")?;
    }

    let (sender, mut receiver) = tokio::sync::mpsc::channel(8);
    let move_string = path.to_owned();
    tokio::spawn(tokio::task::spawn_blocking(move || {
        let output = std::process::Command::new("ffmpeg")
            .args(&["-i", &move_string, "original_frames/%08d.png"])
            .output()
            .expect("FFMPEG probably not installed! ffmpeg command missing.")
            .stderr;
        let data = String::from_utf8_lossy(&output);
        debug!("FFMPEG Video -> Frames stderr:\n{:#?}", data);
        tokio::spawn(async move { sender.send(true).await });
    }));
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    let mut progress = Progress::new();
    let bar: Bar = progress.bar(
        get_original_number_of_frames(&path) as usize,
        "FRAME EXTRACT PROGRESS",
    );
    loop {
        select! {
            _ = interval.tick() => {
                if let Ok(a) = std::fs::read_dir("original_frames"){
                    let amount = a.count();
                    progress.set_and_draw(&bar, amount);
                }
                // println!("Updating the progress bar. {}", progress_bar);
            },
            Some(_) = receiver.recv() => {
                break;
            }
        }
    }
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
        .output()
        .expect("FFMPEG probably not installed! ffmpeg command missing.")
        .stderr;
    let data = String::from_utf8_lossy(&output);
    debug!("FFMPEG Frames -> Video stderr:\n{:#?}", data);
    Ok(())
}

fn dain_process_frames(new_frame_count: u32) -> Result<(), Box<dyn std::error::Error>> {
    if !std::path::Path::new("out_frames").exists() {
        std::fs::create_dir("out_frames")?;
    } else {
        std::fs::remove_dir_all("out_frames")?;
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
        .output()
        .expect("This shouldn't have happened, but apparently DAIN is missing!")
        .stderr;
    let data = String::from_utf8_lossy(&output);
    debug!("DAIN Frames -> Frames stderr:\n{:#?}", data);
    Ok(())
}

fn rife_process_frames(/*new_frame_count: u32*/) -> Result<(), Box<dyn std::error::Error>> {
    if !std::path::Path::new("out_frames").exists() {
        std::fs::create_dir("out_frames")?;
    } else {
        std::fs::remove_dir_all("out_frames")?;
        std::fs::create_dir("out_frames")?;
    }
    #[cfg(not(target_os = "windows"))]
    let command = "./rife/rife-ncnn-vulkan";
    #[cfg(target_os = "windows")]
    let command = "./rife/rife-ncnn-vulkan.exe";
    let output = std::process::Command::new(command)
        .args(&[
            "-i",
            "original_frames",
            "-o",
            "out_frames",
            // "-n",
            // &format!("{}", new_frame_count),
        ])
        .output()
        .expect("This shouldn't have happened, but apparently DAIN is missing!")
        .stderr;
    let data = String::from_utf8_lossy(&output);
    debug!("RIFE Frames -> Frames stderr:\n{:#?}", data);
    Ok(())
}

async fn install_dain() {
    #[cfg(target_os = "windows")]
        let url = "https://github.com/nihui/dain-ncnn-vulkan/releases/download/20210210/dain-ncnn-vulkan-20210210-windows.zip";
    #[cfg(target_os = "windows")]
    let filename = "dain-ncnn-vulkan-20210210-windows";
    #[cfg(target_os = "linux")]
        let url = "https://github.com/nihui/dain-ncnn-vulkan/releases/download/20210210/dain-ncnn-vulkan-20210210-ubuntu.zip";
    #[cfg(target_os = "linux")]
    let filename = "dain-ncnn-vulkan-20210210-ubuntu";
    #[cfg(target_os = "macos")]
        let url = "https://github.com/nihui/dain-ncnn-vulkan/releases/download/20210210/dain-ncnn-vulkan-20210210-macos.zip";
    #[cfg(target_os = "macos")]
    let filename = "dain-ncnn-vulkan-20210210-macos";

    info!("Downloading DAIN...");
    let dain = reqwest::get(url)
        .await
        .unwrap()
        .bytes()
        .await
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
    std::fs::remove_file("dain.zip").unwrap();
    std::fs::rename(filename, "dain").unwrap();
}

async fn install_rife() {
    #[cfg(target_os = "windows")]
        let url = "https://github.com/nihui/rife-ncnn-vulkan/releases/download/20210520/rife-ncnn-vulkan-20210520-windows.zip";
    #[cfg(target_os = "windows")]
    let filename = "rife-ncnn-vulkan-20210520-windows";
    #[cfg(target_os = "linux")]
        let url = "https://github.com/nihui/rife-ncnn-vulkan/releases/download/20210520/rife-ncnn-vulkan-20210520-ubuntu.zip";
    #[cfg(target_os = "linux")]
    let filename = "rife-ncnn-vulkan-20210520-ubuntu";
    #[cfg(target_os = "macos")]
        let url = "https://github.com/nihui/rife-ncnn-vulkan/releases/download/20210520/rife-ncnn-vulkan-20210520-macos.zip";
    #[cfg(target_os = "macos")]
    let filename = "rife-ncnn-vulkan-20210520-macos";

    info!("Downloading RIFE...");
    let rife = reqwest::get(url)
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap()
        .to_vec();
    info!("Writing rife.zip...");
    // let mut pos = 0;
    let mut buffer = std::fs::File::create("rife.zip").unwrap();
    buffer.write_all(&rife[..]).unwrap();
    buffer.flush().unwrap();
    let buffer = std::fs::OpenOptions::new()
        .read(true)
        .open("rife.zip")
        .unwrap();
    info!("rife.zip downloaded. Extracting...");
    let mut zip = zip::read::ZipArchive::new(&buffer).unwrap();
    zip.extract(".").unwrap();
    std::fs::remove_file("rife.zip").unwrap();
    std::fs::rename(filename, "rife").unwrap();
}

fn get_original_number_of_frames(path: &str) -> usize {
    info!("Getting original framecount...");
    let args = &[
        "-i", path, "-map", "0:v:0", "-c", "copy", "-f", "null", "tmp",
    ];
    debug!(
        "Command: ffmpeg {}",
        args.iter()
            .fold(String::new(), |all, item| { format!("{} {}", all, item) })
    );
    let output = std::process::Command::new("ffmpeg")
        .args(&[
            "-i", path, "-map", "0:v:0", "-c", "copy", "-f", "null", "tmp",
        ])
        .output()
        .expect("This shouldn't have happened, but apparently DAIN is missing!")
        .stderr;
    let data = String::from_utf8_lossy(&output);
    let frames = data.lines().nth_back(1).unwrap();
    debug!("FFMPEG Frame collection output:\n {}", frames);
    let frames = frames
        .split("\r")
        .last()
        .unwrap()
        .split("frame=")
        .nth(1)
        .unwrap()
        .split(" fps")
        .nth(0)
        .unwrap()
        .parse::<usize>()
        .unwrap();
    info!("Original framecount is {}", frames);
    frames
}
