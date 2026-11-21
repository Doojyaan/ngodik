use super::*;

#[test]
fn endpoint_checks_transport_and_path() {
    let base = endpoint::base_url(" https://example.test/vendor/v1/ ").unwrap();
    assert_eq!(
        base.join("models").unwrap().as_str(),
        "https://example.test/vendor/v1/models"
    );
    for invalid in [
        "http://example.test/v1",
        "https://name:pass@example.test",
        "https://@example.test",
        "https://example.test/v1?q=1",
        "https://example.test/v1#test",
        "file:///tmp",
    ] {
        assert!(endpoint::base_url(invalid).is_err(), "{invalid}");
    }
    assert!(endpoint::base_url("http://127.0.0.1:9000/v1").is_ok());
    assert!(endpoint::base_url("http://[::1]:9000/v1").is_ok());
}

#[test]
fn old_projects_keep_data_and_have_at_most_one_model() {
    let path = std::env::temp_dir().join(format!("ngodik-v1-{}.sqlite3", uuid::Uuid::new_v4()));
    let original = rusqlite::Connection::open(&path).unwrap();
    original
        .execute_batch(include_str!("../../migrations/001_projects.sql"))
        .unwrap();
    original
        .execute(
            "INSERT INTO projects(title, idea, updated_at) VALUES ('Lama', 'Ide lama', 123)",
            [],
        )
        .unwrap();
    drop(original);
    let store = ProjectStore::open(&path).unwrap();
    assert!(read_selection(&store, 1).unwrap().is_none());
    let profile = write_profile(&store, None, "Satu", "https://example.test/v1").unwrap();
    let other = write_profile(&store, None, "Dua", "https://other.test/v1").unwrap();
    save_selection(&store, 1, &profile.id, "manual-1").unwrap();
    save_selection(&store, 1, &other.id, "manual-2").unwrap();
    assert_eq!(
        read_selection(&store, 1).unwrap().unwrap().model_id,
        "manual-2"
    );
    assert!(matches!(
        write_profile(&store, Some(&other.id), "Dua", "https://changed.test/v1"),
        Err(ProviderError::EndpointLocked)
    ));
    store
        .connection
        .execute("DELETE FROM provider_profiles WHERE id=?1", [&other.id])
        .unwrap();
    assert!(read_selection(&store, 1).unwrap().is_none());
    let idea: String = store
        .connection
        .query_row("SELECT idea FROM projects WHERE id=1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(idea, "Ide lama");
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn credential_manager_rotates_and_deletes_in_its_own_namespace() {
    struct Entry(String);
    impl Drop for Entry {
        fn drop(&mut self) {
            let _ = credentials::delete(&self.0);
        }
    }

    let entry = Entry(credentials::target(
        "id.ngodik.test",
        &Uuid::new_v4().to_string(),
    ));
    assert!(credentials::read(&entry.0).unwrap().is_none());
    let mut marker = Zeroizing::new(b"stage2-fixture-key-first".to_vec());
    credentials::write(&entry.0, &mut marker).unwrap();
    assert!(
        credentials::read(&entry.0)
            .unwrap()
            .is_some_and(|stored| *stored == *marker)
    );
    let mut next = Zeroizing::new(b"stage2-fixture-key-rotated".to_vec());
    credentials::write(&entry.0, &mut next).unwrap();
    assert!(
        credentials::read(&entry.0)
            .unwrap()
            .is_some_and(|stored| *stored == *next)
    );
    credentials::delete(&entry.0).unwrap();
    assert!(credentials::read(&entry.0).unwrap().is_none());
}

fn fixture(
    status: &str,
    extra: &str,
    body: &str,
    pause: std::time::Duration,
) -> (Url, std::thread::JoinHandle<String>) {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::Duration,
    };

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = endpoint::base_url(&format!(
        "http://{}/vendor/v1",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let response = format!(
        "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}\r\n{body}",
        body.len()
    );
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 4096];
        while let Ok(count) = stream.read(&mut buffer) {
            if count == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..count]);
            let text = String::from_utf8_lossy(&request);
            if let Some((headers, body)) = text.split_once("\r\n\r\n") {
                let length = headers
                    .lines()
                    .filter_map(|line| line.split_once(':'))
                    .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, length)| length.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if body.len() >= length {
                    break;
                }
            }
        }
        std::thread::sleep(pause);
        let _ = stream.write_all(response.as_bytes());
        String::from_utf8(request).unwrap()
    });
    (base, thread)
}

#[test]
fn endpoint_fixture_handles_listing_chat_auth_redirect_and_echo() {
    use std::time::Duration;
    const KEY: &[u8] = b"local-fixture-key";
    tauri::async_runtime::block_on(async {
        let client = endpoint::client().unwrap();
        let call = |status, headers, body| fixture(status, headers, body, Duration::ZERO);
        let (base, request) = call("200 OK", "", r#"{"data":[{"id":"alpha"},{"id":"alpha"}]}"#);
        assert_eq!(
            endpoint::models(&client, &base, KEY).await.unwrap(),
            ["alpha"]
        );
        let request = request.join().unwrap();
        assert!(request.starts_with("GET /vendor/v1/models "));
        assert!(
            request
                .to_lowercase()
                .contains("authorization: bearer local-fixture-key")
        );

        let (base, request) = call(
            "200 OK",
            "",
            r#"{"choices":[{"message":{"role":"assistant","content":"OK"}}]}"#,
        );
        endpoint::check(&client, &base, KEY, "manual-model")
            .await
            .unwrap();
        let request = request.join().unwrap();
        assert!(request.starts_with("POST /vendor/v1/chat/completions "));
        assert!(request.contains("manual-model") && request.contains("Reply with OK."));

        for (status, headers, body, expected) in [
            ("404 Not Found", "", "{}", "unavailable_route"),
            ("401 Unauthorized", "", "local-fixture-key", "unauthorized"),
            (
                "302 Found",
                "Location: http://127.0.0.1:1/\r\n",
                "{}",
                "redirect_blocked",
            ),
            (
                "200 OK",
                "",
                r#"{"data":[{"id":"echo-local-fixture-key"}]}"#,
                "invalid_response",
            ),
        ] {
            let (base, request) = call(status, headers, body);
            let error = endpoint::models(&client, &base, KEY).await.unwrap_err();
            assert_eq!(serde_json::to_value(error).unwrap(), expected);
            request.join().unwrap();
        }
    });
}

#[test]
fn endpoint_fixture_reports_timeout() {
    use std::time::Duration;
    let (base, request) = fixture("200 OK", "", "{}", Duration::from_millis(450));
    tauri::async_runtime::block_on(async {
        let client = endpoint::client_with_timeout(Duration::from_millis(180)).unwrap();
        assert!(matches!(
            endpoint::models(&client, &base, b"local-fixture-key").await,
            Err(ProviderError::Timeout)
        ));
    });
    request.join().unwrap();
}
