use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time;

use anyhow::anyhow;
use anyhow::Result;
use bytes::Bytes;
use ffmpeg_next::codec::Parameters;
use ffmpeg_next::encoder::{Encoder, Video};
use scap::Target;
use scap::{
    capturer::{Area, Capturer, Options, Point, Size},
    frame::Frame,
};
use tokio::sync::{mpsc, Mutex};
use tokio::time::Duration;
use tokio_util::codec::{BytesCodec, FramedRead};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_VP8};
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::media::io::ivf_reader::IVFReader;
use webrtc::media::Sample;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;
use webrtc::Error;

use tokio::runtime::Runtime;

// Re-exports to shorten ffmpeg calls
use ffmpeg::codec::packet::Packet;
use ffmpeg::codec::{self, Context as CodecContext};
use ffmpeg::format::Pixel;
use ffmpeg::software::scaling::{flag::Flags, Context as SwsContext};
use ffmpeg::util::frame::video::Video as FFrame;
use ffmpeg_next as ffmpeg;

// Add a single video track
pub async fn add_video(pc: &Arc<RTCPeerConnection>) -> Result<()> {
    // let video_track = Arc::new(TrackLocalStaticSample::new(
    //     RTCRtpCodecCapability {
    //         mime_type: MIME_TYPE_VP8.to_owned(),
    //         ..Default::default()
    //     },
    //     format!("video-{}", rand::random::<u32>()),
    //     format!("video-{}", rand::random::<u32>()),
    // ));

    // ----------------------------
    // Suppose you already have a webrtc-rs TrackLocalStaticSample
    // We'll just mock it here as `t`. In real code, you set it up properly.
    // ----------------------------
    let h264_codec_cap = webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability {
        mime_type: "video/h264".to_string(),
        ..Default::default()
    };

    let video_track = Arc::new(TrackLocalStaticSample::new(
        h264_codec_cap,
        format!("video-{}", rand::random::<u32>()),
        format!("video-{}", rand::random::<u32>()),
    ));

    let rtp_sender = match pc
        .add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
        .await
    {
        Ok(rtp_sender) => rtp_sender,
        Err(err) => panic!("{}", err),
    };

    // Read incoming RTCP packets
    // Before these packets are returned they are processed by interceptors. For things
    // like NACK this needs to be called.
    tokio::spawn(async move {
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
        Result::<()>::Ok(())
    });

    {
        tokio::spawn(async move {
            let res =
                // write_video_to_track2("/Users/wgmlgz/websh/output.ivf".into(), video_track).await;
                write_video_to_track( video_track).await;
            dbg!(res);
        });
    }

    println!("Video track has been added");
    Ok(())
}

// Remove a single sender
async fn remove_video(pc: &Arc<RTCPeerConnection>) -> Result<()> {
    let senders = pc.get_senders().await;
    if !senders.is_empty() {
        if let Err(err) = pc.remove_track(&senders[0]).await {
            panic!("{}", err);
        }
    }

    println!("Video track has been removed");
    Ok(())
}

// Read a video file from disk and write it to a webrtc.Track
// When the video has been completely read this exits without error
pub async fn write_video_to_track(t: Arc<TrackLocalStaticSample>) -> Result<()> {
    // Check if the platform is supported
    if !scap::is_supported() {
        println!("❌ Platform not supported");
        return Err(anyhow!("Platform not supported"));
    }

    // Check if we have permission to capture screen
    // If we don't, request it.
    if !scap::has_permission() {
        println!("❌ Permission not granted. Requesting permission...");
        if !scap::request_permission() {
            println!("❌ Permission denied");
            return Err(anyhow!("Permission denied"));
        }
    }

    // Get recording targets
    let targets = scap::get_all_targets();
    let targets = targets
        .into_iter()
        .filter(|target| {
            if let Target::Display(_) = target {
                return true;
            }
            return false;
        })
        .collect::<Vec<_>>();

    println!("Targets: {:?}", targets);

    // Create Options
    let options = Options {
        fps: 60,
        target: None, // None captures the primary display
        show_cursor: true,
        show_highlight: true,
        excluded_targets: None,
        output_type: scap::frame::FrameType::BGRAFrame,
        output_resolution: scap::capturer::Resolution::Captured,

        // source_rect: Some(Area {
        //     origin: Point { x: 0.0, y: 0.0 },
        //     size: Size {
        //         width: 2000.0,
        //         height: 1000.0,
        //     },
        // }),
        ..Default::default()
    };

    // Create Capturer
    let mut capturer = Capturer::new(options);

    // Start Capture
    capturer.start_capture();

    let mut start_time: u64 = 0;
    println!("impostor");

    println!("amogus");

    let (tx, mut rx) = mpsc::channel::<Bytes>(1);

    // tokio::spawn(async move {
    //     let mut i = 0;
    //     println!("waiting frame");
    //     while let Some(data) = rx.recv().await {
    //         println!("got frame");

    //         t.write_sample(&Sample {
    //             data,
    //             duration: Duration::from_secs(1),
    //             packet_timestamp: i,
    //             ..Default::default()
    //         })
    //         .await
    //         .unwrap();
    //         dbg!("wrote frame");
    //         i += 1;
    //     }
    // });

    // tokio::task::spawn_blocking(move || {
    //     for i in 0.. {
    //         let res = capturer.get_next_frame();
    //         let Ok(frame) = res else {
    //             break;
    //         };

    //         let Frame::BGRA(frame) = frame else {
    //             println!("unknown format recieved");
    //             continue;
    //         };
    //         if start_time == 0 {
    //             start_time = frame.display_time;
    //         }

    //         tx.blocking_send(frame.data.into()).unwrap();
    //         dbg!("sent frame");
    //         // println!(
    //         //     "Recieved BGRA frame {} of width {} and height {} and time {}",
    //         //     i,
    //         //     frame.width,
    //         //     frame.height,
    //         //     frame.display_time - start_time
    //         // );
    //     }
    // });

    // Channel from encoding thread => WebRTC writer
    let (tx, mut rx) = mpsc::channel::<Bytes>(16);

    // ----------------------------
    // 1) Async task that receives H.264 packets, writes them to the WebRTC track
    // ----------------------------
    let track_clone = Arc::clone(&t);
    tokio::spawn(async move {
        let mut packet_timestamp = 0u64;
        println!("Awaiting encoded frames...");

        while let Some(h264_nal) = rx.recv().await {
            println!("Got an H.264 packet, sending to track...");

            track_clone
                .write_sample(&Sample {
                    data: h264_nal,
                    // For a real 30fps scenario, 33ms per frame
                    duration: Duration::from_millis(33),
                    packet_timestamp: packet_timestamp.try_into().unwrap(),
                    ..Default::default()
                })
                .await
                .expect("Failed to write sample");

            packet_timestamp += 1;
        }
        println!("Channel closed, no more frames.");
    });

    // ----------------------------
    // 2) Blocking task that captures BGRA frames, converts + encodes them with FFmpeg, sends packets
    // ----------------------------
    tokio::task::spawn_blocking(move || {
        // For example, let's assume 1280x720
        let width = 4112;
        let height = 2658;

        // Initialize FFmpeg stuff
        let mut encoder =
            init_ffmpeg_encoder(width, height).expect("Failed to init ffmpeg encoder");
        let mut scaler = init_scaler(width, height).expect("Failed to init scaler");

        for i in 0.. {
            // capture ~300 frames as a demo
            // 1) Grab a BGRA frame from your real screen capturer.
            //    Here, we just mock it:

            //     for i in 0.. {
            let res = capturer.get_next_frame();
            // let frame = ScreenFrame {
            //     data: res.
            // };
            let Ok(frame) = res else {
                break;
            };

            let Frame::BGRA(frame) = frame else {
                println!("unknown format recieved");
                continue;
            };
            dbg!(frame.width);
            dbg!(frame.height);
            let frame = ScreenFrame {
                width,
                height,
                data: frame.data,
            };
    //         if start_time == 0 {
    //             start_time = frame.display_time;
    //         }

    //         tx.blocking_send(frame.data.into()).unwrap();
            // let frame = mock_bgra_capture(width, height);
            // let Some(frame) = frame else {
            //     println!("No more frames from capture.");
            //     break;
            // };

            // 2) Encode that BGRA frame to H.264
            let encoded = encode_bgra_frame(
                &frame.data,
                &mut encoder,
                &mut scaler,
                frame.width,
                frame.height,
            )
            .expect("Failed to encode frame");

            // 3) Send each encoded packet over the channel
            for packet in encoded {
                // This is a blocking send because we are in spawn_blocking
                tx.blocking_send(packet).unwrap();
            }

            println!("Captured & encoded frame #{}", i);
        }

        // We must flush the encoder: send_frame(None)
        {
            let mut encoder = encoder;
            encoder.send_eof().ok();
            loop {
                let mut packet = Packet::empty();
                match encoder.receive_packet(&mut packet) {
                    Ok(_) => {
                        let v = Vec::from(packet.data().unwrap());
                        // let data = .to_owned().unwrap();
                        tx.blocking_send(Bytes::from(v)).unwrap();
                    }
                    Err(e) if e == ffmpeg::Error::Eof => {
                        break;
                    }
                    Err(e) => {
                        eprintln!("Error flushing encoder: {:?}", e);
                        break;
                    }
                }
            }
        }

        println!("Done capturing/encoding. Closing channel.");
        // Closing tx ends the while-let in the async task
    });

    // For demo purposes, just sleep a bit or run your real app loop
    // tokio::time::sleep(Duration::from_secs(10)).await;
    // Ok(())

    // let mut input = String::new();
    // std::io::stdin().read_line(&mut input).unwrap();
    // Stop Capture
    // capturer.stop_capture();

    Ok(())
    // println!("play video from disk file {video_file}");

    // // Open a IVF file and start reading using our IVFReader
    // let file = File::open(video_file)?;
    // let reader = BufReader::new(file);
    // let (mut ivf, header) = IVFReader::new(reader)?;

    // // It is important to use a time.Ticker instead of time.Sleep because
    // // * avoids accumulating skew, just calling time.Sleep didn't compensate for the time spent parsing the data
    // // * works around latency issues with Sleep
    // // Send our video file frame at a time. Pace our sending so we send it at the same speed it should be played back as.
    // // This isn't required since the video is timestamped, but we will such much higher loss if we send all at once.
    // let sleep_time = Duration::from_millis(
    //     ((1000 * header.timebase_numerator) / header.timebase_denominator) as u64,
    // );
    // let mut ticker = tokio::time::interval(sleep_time);
    // loop {
    //     let frame = match ivf.parse_next_frame() {
    //         Ok((frame, _)) => frame,
    //         Err(err) => {
    //             println!("All video frames parsed and sent: {err}");
    //             return Err(err.into());
    //         }
    //     };

    //     t.write_sample(&Sample {
    //         data: frame.freeze(),
    //         duration: Duration::from_secs(1),
    //         ..Default::default()
    //     })
    //     .await?;

    //     let _ = ticker.tick().await;
    // }
}

// Read a video file from disk and write it to a webrtc.Track
// When the video has been completely read this exits without error
async fn write_video_to_track2(video_file: String, t: Arc<TrackLocalStaticSample>) -> Result<()> {
    println!("play video from disk file {video_file}");

    // Open a IVF file and start reading using our IVFReader
    let file = File::open(video_file)?;
    let reader = BufReader::new(file);
    let (mut ivf, header) = IVFReader::new(reader)?;

    // It is important to use a time.Ticker instead of time.Sleep because
    // * avoids accumulating skew, just calling time.Sleep didn't compensate for the time spent parsing the data
    // * works around latency issues with Sleep
    // Send our video file frame at a time. Pace our sending so we send it at the same speed it should be played back as.
    // This isn't required since the video is timestamped, but we will such much higher loss if we send all at once.
    let sleep_time = Duration::from_millis(
        ((1000 * header.timebase_numerator) / header.timebase_denominator) as u64,
    );
    let mut ticker = tokio::time::interval(sleep_time);
    loop {
        // dbg!("sus");
        let frame = match ivf.parse_next_frame() {
            Ok((frame, _)) => frame,
            Err(err) => {
                println!("All video frames parsed and sent: {err}");
                return Err(err.into());
            }
        };

        // dbg!(&frame);

        t.write_sample(&Sample {
            data: frame.freeze(),
            duration: Duration::from_secs(1),
            ..Default::default()
        })
        .await?;

        let _ = ticker.tick().await;
    }
}

/// Mock struct for demonstration. Replace with your real screen capture.
struct ScreenFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // BGRA bytes
}

/// Example capture function returning BGRA frames.
/// In real life, you'd have something like `capturer.get_next_frame()`.
fn mock_bgra_capture(width: u32, height: u32) -> Option<ScreenFrame> {
    // BGRA is 4 bytes/pixel
    let size = (width * height * 4) as usize;
    let fake_bgra = vec![0u8; size]; // all zeros => black frame
    Some(ScreenFrame {
        width,
        height,
        data: fake_bgra,
    })
}

/// Initialize the FFmpeg library, create an H.264 encoder context.
fn init_ffmpeg_encoder(width: u32, height: u32) -> Result<Video> {
    ffmpeg::init()?;

    let codec =
        ffmpeg::encoder::find(codec::Id::H264).ok_or_else(|| anyhow!("No H.264 encoder found"))?;

    let context = CodecContext::new_with_codec(codec);
    let mut video = context.encoder().video()?; // Temporarily borrow mut
    video.set_width((width as i32).try_into().unwrap());
    video.set_height((height as i32).try_into().unwrap());
    video.set_format(Pixel::YUV420P);
    video.set_time_base(ffmpeg::Rational(1, 30));
    video.set_frame_rate(Some(ffmpeg::Rational(30, 1)));
    // let r = {
    let encoder = video.open_as(codec)?;
    // let r = encoder.encoder();
    // r
    // };
    Ok(encoder)
}

/// Create a scaling context to convert BGRA -> YUV420P
fn init_scaler(width: u32, height: u32) -> Result<SwsContext> {
    let scaler = SwsContext::get(
        Pixel::BGRA, // source pixel format
        width,
        height,
        Pixel::YUV420P, // destination pixel format
        width,
        height,
        Flags::BILINEAR, // or use FAST_BILINEAR, etc.
    )?;
    Ok(scaler)
}

/// Encode a single BGRA frame to H.264 using FFmpeg.
/// Returns a `Vec<Bytes>` because FFmpeg may produce multiple packets for a single frame (e.g., keyframes).
fn encode_bgra_frame(
    bgra_data: &[u8],
    encoder: &mut Encoder,
    scaler: &mut SwsContext,
    width: u32,
    height: u32,
) -> Result<Vec<Bytes>> {
    // 1) Create a temporary ffmpeg "Frame" in BGRA
    //    Actually, we create a YUV frame, but first we allocate enough space
    let mut yuv_frame = FFrame::new(Pixel::YUV420P, width, height);

    // 2) Create another ffmpeg "Frame" to hold the BGRA data for swscale
    let mut bgra_frame = FFrame::new(Pixel::BGRA, width, height);

    // Fill the BGRA frame's data planes
    {
        let data_0 = bgra_frame.data_mut(0); // Plane #0 for BGRA
                                             // let stride_0 = bgra_frame.stride(0);     // Typically width * 4 for BGRA

        // Copy from your raw BGRA bytes into the frame’s plane
        // Make sure the input data size fits within data_0
        data_0[..bgra_data.len()].copy_from_slice(bgra_data);
    }

    // 3) Convert BGRA -> YUV420P using swscale
    scaler.run(&bgra_frame, &mut yuv_frame)?;

    // 4) Send the converted YUV frame to the encoder
    let mut encoded_packets = Vec::new();
    encoder.send_frame(&yuv_frame)?;

    // 5) Read all produced packets
    loop {
        let mut packet = Packet::empty();
        match encoder.receive_packet(&mut packet) {
            Ok(_) => {
                // Move packet data to Bytes
                let v = Vec::from(packet.data().unwrap());
                // let pkt_data = packet.data().to_owned();
                // if let Some(data) = pkt_data {
                dbg!("sus");
                encoded_packets.push(Bytes::from(v));
                // }
            }
            Err(e) if e == ffmpeg::Error::Eof => {
                break;
            }
            Err(e) => {
                break;
                return Err(anyhow::anyhow!("Error receiving packet: {:?}", e));
            }
        }
    }

    Ok(encoded_packets)
}

// #[tokio::main]
// async fn main() -> Result<()> {

// }
