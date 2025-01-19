use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{self, Instant};

use anyhow::anyhow;
use anyhow::Result;
use bytes::Bytes;
use ffmpeg_next::threading::Config;
use ffmpeg_next::util::format;
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc, Mutex};
use tokio::time::Duration;
use tokio_util::codec::{BytesCodec, FramedRead};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_VP8, MIME_TYPE_VP9};
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

use ffmpeg::codec::packet::Packet;
use ffmpeg::codec::{self, Context as CodecContext};
use ffmpeg::format::Pixel;
use ffmpeg::software::scaling::{flag::Flags, Context as SwsContext};
use ffmpeg::util::frame::video::Video as FFrame;
use ffmpeg_next::codec::{Id, Parameters};
use ffmpeg_next::encoder::{Encoder, Video};
use ffmpeg_next::{self as ffmpeg, Dictionary, Rational};

use tokio::runtime::Runtime;

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
    // let h264_codec_cap = webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability {
    //     mime_type: "video/h264".to_string(),
    //     ..Default::default()
    // };

    let video_track = Arc::new(TrackLocalStaticSample::new(
        // h264_codec_cap,
        RTCRtpCodecCapability {
            mime_type:
            //  if is_vp9 {
            //     MIME_TYPE_VP9.to_owned()
            // } else {
                MIME_TYPE_VP8.to_owned(),
            // },
            ..Default::default()
        },
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
            let res = write_video_to_track2(video_track).await;
            // write_video_to_track( video_track).await;
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

pub fn record(tx: Sender<Bytes>) -> Result<()> {
    use scrap::{Capturer, Display};
    use std::io::ErrorKind::WouldBlock;
    use std::io::Write;
    use std::process::{Command, Stdio};

    let d = Display::primary().unwrap();
    let (w, h) = (d.width(), d.height());

    // let child = Command::new("ffplay")
    //     .args(&[
    //         "-f",
    //         "rawvideo",
    //         "-pixel_format",
    //         "bgr0",
    //         "-video_size",
    //         &format!("{}x{}", w, h),
    //         "-framerate",
    //         "60",
    //         "-",
    //     ])
    //     .stdin(Stdio::piped())
    //     .spawn()
    //     .expect("This example requires ffplay.");

    let mut capturer = Capturer::new(d).unwrap();
    // let mut out = child.stdin.unwrap();

    // let mut dest = Vec::new();

    let start = Instant::now();

    let mut scaler = init_scaler(w as u32, h as u32).expect("Failed to init scaler");
    let mut encoder =
        init_vp8_encoder(w as u32, h as u32, 500000).expect("Failed to initialize VP8 encoder");

    loop {
        let now = Instant::now();
        let time = now - start;

        match capturer.frame() {
            Ok(frame) => {
                let ms = time.as_secs() * 1000 + time.subsec_millis() as u64;
                let mut yuv_frame = FFrame::new(Pixel::YUV420P, (w / 2) as u32, (h / 2) as u32);
                let mut bgra_frame = FFrame::new(Pixel::BGRA, w as u32, h as u32);

                {
                    let data_0 = bgra_frame.data_mut(0); // Plane #0 for BGRA
                                                         // let stride_0 = bgra_frame.stride(0);     // Typically width * 4 for BGRA

                    // Copy from your raw BGRA bytes into the frame’s plane
                    // Make sure the input data size fits within data_0
                    data_0[..frame.len()].copy_from_slice(&frame);
                };
                // dbg!(yuv_frame.width(), yuv_frame.height());
                // 3) Convert BGRA -> YUV420P using swscale
                scaler.run(&bgra_frame, &mut yuv_frame)?;

                encoder.send_frame(&yuv_frame)?;

                loop {
                    let mut packet = Packet::empty();
                    match encoder.receive_packet(&mut packet) {
                        Ok(_) => {
                            let v = Vec::from(packet.data().unwrap());
                            tx.blocking_send(Bytes::from(v)).unwrap();
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
            }
            Err(ref e) if e.kind() == WouldBlock => {
                // Wait for the frame.
            }
            Err(_) => {
                // We're done here.
                break;
            }
        }
    }
    return Ok(());
}

// Read a video file from disk and write it to a webrtc.Track
// When the video has been completely read this exits without error
async fn write_video_to_track2(t: Arc<TrackLocalStaticSample>) -> Result<()> {
    let (tx, mut rx) = mpsc::channel::<Bytes>(100);

    tokio::task::spawn_blocking(|| {
        record(tx).unwrap();
    });
    loop {
        let frame = rx.recv().await;
        let Some(frame) = frame else {
            break;
        };

        t.write_sample(&Sample {
            // data: frame.freeze(),
            data: frame,
            duration: Duration::from_millis(1),
            ..Default::default()
        })
        .await?;
    }
    Ok(())
}

fn init_scaler(width: u32, height: u32) -> Result<SwsContext> {
    dbg!(width, height);
    let scaler = SwsContext::get(
        Pixel::BGRA, // source pixel format
        width,
        height,
        Pixel::YUV420P, // destination pixel format
        width / 2,
        height / 2,
        Flags::BILINEAR, // or use FAST_BILINEAR, etc.
    )?;
    Ok(scaler)
}

fn init_vp8_encoder(width: u32, height: u32, bitrate: usize) -> Result<Video> {
    // Find the VP8 codec
    let codec = ffmpeg::encoder::find(Id::VP8).ok_or_else(|| anyhow!("No H.264 encoder found"))?;

    let context = CodecContext::new_with_codec(codec);
    let mut video = context.encoder().video()?; // Temporarily borrow mut
    dbg!("a");
    video.set_width(((width / 2) as i32).try_into().unwrap());
    video.set_height(((height / 2) as i32).try_into().unwrap());
    video.set_format(Pixel::YUV420P);
    video.set_time_base(Rational(1, 120));
    video.set_bit_rate(100000000);
    video.set_threading(Config {
        kind: codec::threading::Type::Frame,
        count: 0,
    });
    // video.set_max_bit_rate(0);
    // video.set_quality(60);
    // video.set_global_quality(60);
    let mut dict = Dictionary::new();
    // dict.set("crf", "60");       // Properly formatted bitrate
    // dict.set("bitrate", "10M");       // Properly formatted bitrate
    // dict.set("-bitrate", "10M");       // Properly formatted bitrate
    // dict.set("-b:v", "10M");       // Properly formatted bitrate
    // dict.set("crf", "10");                         // Highest quality / lossless

    // video.set_parameters(dict);
    // video.set_global_quality(0);
    // video.set_quality(0);
    // video.set_parameters("crf", "0")?;   // Set CRF to 0 for highest quality (lossless)
    // video.set_option("b:v", &format!("{}", bitrate))?; // Set desired bitrate
    // video.set_bit_rate(bitrate);

    // Set pixel format expected by VP8 encoder
    // context.set_format(Pixel::YUV420P);

    dbg!("b");

    dbg!("b");
    let mut encoder = video.open_as_with(codec, dict)?;
    dbg!("c");

    // let r = encoder.encoder();
    // r
    // };
    // encoder.set_bit_rate(bitrate);
    Ok(encoder)
}
