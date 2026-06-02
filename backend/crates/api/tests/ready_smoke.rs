//! End-to-end READY-handshake smoke test (D1 · backend/01 §1.1.1).
//!
//! Spawns the real `stuchka-core` binary as a child process, scans its stdout for the single
//! `READY{"port":N,"token":"<hex>"}` line, builds an HTTP client exactly as the Flutter parent
//! would, and asserts `GET /health` with the Bearer token returns 200. Also asserts that a
//! request *without* the token is rejected, proving the live server enforces auth.
//!
//! The child is bound to `127.0.0.1:0` so it never collides with other ports, and is killed at
//! the end of the test.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Ready {
    port: u16,
    token: String,
}

/// Read lines from the child's stdout until one starts with `READY`, then parse the JSON.
fn read_ready(child_stdout: std::process::ChildStdout) -> Ready {
    let mut reader = BufReader::new(child_stdout);
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).expect("read child stdout");
        assert!(n > 0, "child closed stdout before emitting READY");
        if let Some(json) = line.trim_end().strip_prefix("READY") {
            return serde_json::from_str(json).expect("parse READY json");
        }
    }
}

#[test]
fn ready_then_health_200_with_bearer() {
    // `CARGO_BIN_EXE_stuchka-core` is set by cargo for the integration test runner.
    let bin = env!("CARGO_BIN_EXE_stuchka-core");
    let mut child = Command::new(bin)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn stuchka-core");

    let stdout = child.stdout.take().expect("child stdout piped");
    let ready = read_ready(stdout);
    assert!(ready.port > 0, "READY port must be nonzero");
    assert_eq!(ready.token.len(), 128, "token is 64 bytes hex = 128 chars");

    let base = format!("http://127.0.0.1:{}", ready.port);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .no_proxy()
        .build()
        .expect("build client");

    // Give the server a brief moment to start serving after printing READY.
    let mut last_status = None;
    for _ in 0..50 {
        match client
            .get(format!("{base}/health"))
            .bearer_auth(&ready.token)
            .send()
        {
            Ok(resp) => {
                last_status = Some(resp.status().as_u16());
                if resp.status().is_success() {
                    break;
                }
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    assert_eq!(
        last_status,
        Some(200),
        "GET /health with Bearer must be 200"
    );

    // Without the token the live server must reject (401).
    let unauth = client
        .get(format!("{base}/health"))
        .send()
        .expect("send unauth");
    assert_eq!(
        unauth.status().as_u16(),
        401,
        "missing Bearer => 401 on live server"
    );

    // A wired route returns the real contract envelope (GET /kb/version is live after wire1).
    let kbv = client
        .get(format!("{base}/kb/version"))
        .bearer_auth(&ready.token)
        .send()
        .expect("send kb/version");
    assert_eq!(kbv.status().as_u16(), 200, "wired kb/version => 200");
    let kbv_body: serde_json::Value = kbv.json().expect("json envelope");
    assert!(kbv_body["error"].is_null());
    assert!(
        kbv_body["data"]["versionHash"]
            .as_str()
            .map(|h| h.len() == 64)
            .unwrap_or(false),
        "kb version hash is 64-hex: {kbv_body}"
    );

    // A genuinely-R1b route still returns 501 E_NOT_IMPLEMENTED (POST /audit/anchor — the real
    // OpenTimestamps calendar upload is R1b, W8). Confirms the skeleton fallback is intact for the
    // routes that legitimately defer. (`GET /document/:id` is wired in wire2 and no longer 501.)
    let ni = client
        .post(format!("{base}/audit/anchor"))
        .bearer_auth(&ready.token)
        .send()
        .expect("send audit/anchor");
    assert_eq!(ni.status().as_u16(), 501, "R1b route => 501");
    let body: serde_json::Value = ni.json().expect("json envelope");
    assert_eq!(body["error"]["code"], "E_NOT_IMPLEMENTED");
    assert!(body["data"].is_null());

    child.kill().expect("kill child");
    let _ = child.wait();
}
