use std::{f32::consts::PI, str::FromStr, time::Duration};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, Sample, SampleFormat, StreamConfig,
};
use iroh::{PublicKey, SecretKey};
use shamble_core::ShambleState;
use tiny_http::Response;
use tracing::{debug, error, info};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::try_from_default_env().unwrap_or(
                EnvFilter::from_str(&format!("host=debug,shamble_core=debug",)).unwrap(),
            ),
        )
        .init();

    // read or gen private key
    let private_fname = std::env::args().nth(1).unwrap_or("private.key".into());
    if !std::path::Path::new(&private_fname).exists() {
        let mut rng = rand::rngs::OsRng;
        let private: SecretKey = SecretKey::generate(&mut rng);
        std::fs::write(&private_fname, private.to_string()).unwrap();
    }
    let private: SecretKey =
        SecretKey::from_str(&std::fs::read_to_string(&private_fname).unwrap()).unwrap();

    // set up runtime and connection

    let state = ShambleState::new(&private);
    let pub_key = private.public();
    println!("Public Key: {}", pub_key);

    // listen for commands
    let commander = state.command();
    tokio::task::spawn_blocking(move || {
        let port = std::env::args().nth(2).unwrap_or("8787".into());
        let server = tiny_http::Server::http(format!("127.0.0.1:{port}")).unwrap();
        loop {
            // blocks until the next request is received
            let request = match server.recv() {
                Ok(rq) => rq,
                Err(e) => {
                    println!("error: {}", e);
                    break;
                }
            };
            let (url, params) = request.url().split_once("?").unwrap_or((request.url(), ""));

            match url {
                "/connect" => match PublicKey::from_str(params) {
                    Ok(c) => {
                        commander
                            .blocking_send(shamble_core::Command::ConnectTo(c))
                            .map_err(|e| error!(?e, "commanding"))
                            .ok();
                        info!("Connecting to {params:?}");
                    }
                    Err(e) => {
                        request
                            .respond(
                                Response::from_string(format!("Error decoding address: {e:?}"))
                                    .with_status_code(400),
                            )
                            .ok();
                    }
                },
                r => info!("Invalid request {r:?}"),
            }
        }
    })
    .await
    .unwrap();

    // FIXME: graceful shutdown
}

// fn main() {
//     let h = cpal::default_host();
//     let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);
//     for d in h.output_devices().unwrap() {
//         let dname = d.name().unwrap();
//         println!("Initializing {dname}");
//         let supported_config = d.default_output_config().unwrap();
//         let sample_format = supported_config.sample_format();
//         let c: StreamConfig = supported_config.into();
//         let stream = match sample_format {
//             SampleFormat::F32 => d.build_output_stream(&c, write_sin::<f32>, err_fn, None),
//             SampleFormat::I16 => d.build_output_stream(&c, write_sin::<i16>, err_fn, None),
//             SampleFormat::U16 => d.build_output_stream(&c, write_sin::<u16>, err_fn, None),
//             SampleFormat::I32 => d.build_output_stream(&c, write_sin::<i32>, err_fn, None),
//             sample_format => panic!("Unsupported sample format '{sample_format}'"),
//         }
//         .unwrap();
//         stream.play().unwrap();
//         println!("Beeping at {dname} as {sample_format}");
//         std::thread::sleep(Duration::from_secs(1));
//     }
// }

// fn write_sin<T: Sample + FromSample<f32>>(data: &mut [T], _: &cpal::OutputCallbackInfo) {
//     let mut sin_phase: f32 = 0.0;
//     let dp = 880.0 / 44100.0 * 2.0 * PI;
//     for sample in data.iter_mut() {
//         *sample = Sample::from_sample(sin_phase.sin() * 0.5);
//         sin_phase += dp;
//     }
// }
