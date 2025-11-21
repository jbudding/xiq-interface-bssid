mod db;
mod parser;

use anyhow::{Context, Result};
use db::Database;
use parser::extract_interfaces;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::File;
use std::io::Write;

/// Escape a string for CSV output (RFC 4180 compliant)
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct LoginResponse {
    access_token: String,
}


#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DevicesResponse {
    data: Vec<serde_json::Value>,
    total_pages: Option<i32>,
    total_count: Option<i32>,
    page: Option<i32>,
}

struct CloudIQClient {
    client: reqwest::Client,
    base_url: String,
    access_token: Option<String>,
}

impl CloudIQClient {
    fn new(base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
            access_token: None,
        }
    }

    async fn login(&mut self, username: &str, password: &str) -> Result<()> {
        let login_url = format!("{}/login", self.base_url);

        let login_payload = LoginRequest {
            username: username.to_string(),
            password: password.to_string(),
        };

        let response = self
            .client
            .post(&login_url)
            .json(&login_payload)
            .send()
            .await
            .context("Failed to send login request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Login failed with status {}: {}", status, error_text);
        }

        let login_response: LoginResponse = response
            .json()
            .await
            .context("Failed to parse login response")?;

        self.access_token = Some(login_response.access_token);
        println!("Successfully authenticated with CloudIQ API");

        Ok(())
    }

    async fn get_devices(&self) -> Result<Vec<serde_json::Value>> {
        let token = self
            .access_token
            .as_ref()
            .context("Not authenticated. Please login first.")?;

        let mut all_devices = Vec::new();
        let mut page = 1;
        let limit = 100;

        loop {
            println!("Fetching page {} with limit {}...", page, limit);

            let devices_url = format!(
                "{}/devices?page={}&limit={}&deviceTypes=REAL&async=false",
                self.base_url, page, limit
            );

            let mut headers = HeaderMap::new();
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", token))
                    .context("Failed to create authorization header")?,
            );

            let response = self
                .client
                .get(&devices_url)
                .headers(headers)
                .send()
                .await
                .context("Failed to send devices request")?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                anyhow::bail!("Failed to fetch devices with status {}: {}", status, error_text);
            }

            let devices_response: DevicesResponse = response
                .json()
                .await
                .context("Failed to parse devices response")?;

            let devices_in_page = devices_response.data.len();
            println!("Retrieved {} devices from page {}", devices_in_page, page);

            all_devices.extend(devices_response.data);

            // Check if we have more pages to fetch
            if let Some(total_pages) = devices_response.total_pages {
                if page >= total_pages {
                    println!("Reached last page ({}/{})", page, total_pages);
                    break;
                }
            } else if devices_in_page < limit {
                // If no total_pages info, stop when we get fewer devices than the limit
                println!("Reached last page (received {} devices, less than limit of {})", devices_in_page, limit);
                break;
            }

            page += 1;
        }

        println!("Successfully retrieved {} total devices across all pages", all_devices.len());

        Ok(all_devices)
    }

    async fn save_devices_to_file(&self, filename: &str) -> Result<()> {
        let devices = self.get_devices().await?;

        let json_data = serde_json::to_string_pretty(&devices)
            .context("Failed to serialize devices to JSON")?;

        let mut file = File::create(filename)
            .context(format!("Failed to create file: {}", filename))?;

        file.write_all(json_data.as_bytes())
            .context("Failed to write data to file")?;

        println!("Devices saved to {}", filename);

        Ok(())
    }

    async fn save_devices_to_db(&self, db: &Database) -> Result<()> {
        let devices = self.get_devices().await?;

        // Count devices by device_function
        let total_devices = devices.len();
        let ap_devices = devices.iter()
            .filter(|device| {
                device.get("device_function")
                    .and_then(|v| v.as_str())
                    .map(|s| s == "AP")
                    .unwrap_or(false)
            })
            .count();

        println!("\n=== Device Import Summary ===");
        println!("Total devices imported: {}", total_devices);
        println!("Devices with device_function 'AP': {}", ap_devices);
        println!("============================\n");

        db.insert_devices(&devices).await?;
        Ok(())
    }

    async fn send_cli_command(&self, device_ids: &[i64], command: &str) -> Result<Vec<(i64, String)>> {
        let token = self
            .access_token
            .as_ref()
            .context("Not authenticated. Please login first.")?;

        let cli_url = format!("{}/devices/:cli", self.base_url);

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token))
                .context("Failed to create authorization header")?,
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        let payload = serde_json::json!({
            "devices": {
                "ids": device_ids
            },
            "clis": [command]
        });

        let response = self
            .client
            .post(&cli_url)
            .headers(headers)
            .json(&payload)
            .send()
            .await
            .context("Failed to send CLI command request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("CLI command failed with status {}: {}", status, error_text);
        }

        let response_text = response.text().await.context("Failed to get response text")?;

        let cli_response: serde_json::Value = serde_json::from_str(&response_text)
            .context("Failed to parse CLI response as JSON")?;

        let mut results = Vec::new();
        if let Some(outputs) = cli_response.get("device_cli_outputs").and_then(|v| v.as_object()) {
            for (device_id_str, output_value) in outputs {
                if let Ok(device_id) = device_id_str.parse::<i64>() {
                    // Handle different possible output formats
                    let output = if let Some(arr) = output_value.as_array() {
                        // Array of objects with "output" field
                        arr.iter()
                            .filter_map(|item| {
                                item.get("output").and_then(|o| o.as_str())
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    } else if let Some(s) = output_value.as_str() {
                        s.to_string()
                    } else {
                        output_value.to_string()
                    };
                    results.push((device_id, output));
                }
            }
        }

        Ok(results)
    }

    fn get_connected_aps(devices: &[serde_json::Value]) -> Vec<(i64, String)> {
        devices
            .iter()
            .filter(|device| {
                let connected = device.get("connected")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let is_ap = device.get("device_function")
                    .and_then(|v| v.as_str())
                    .map(|s| s == "AP")
                    .unwrap_or(false);
                connected && is_ap
            })
            .filter_map(|device| {
                let id = device.get("id")?.as_i64()?;
                let hostname = device.get("hostname")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                Some((id, hostname))
            })
            .collect()
    }

    async fn run_command_on_connected_aps(&self, command: &str) -> Result<()> {
        let devices = self.get_devices().await?;
        let connected_aps = Self::get_connected_aps(&devices);

        if connected_aps.is_empty() {
            println!("No connected APs found.");
            return Ok(());
        }

        println!("\n=== Found {} connected APs ===", connected_aps.len());
        for (id, hostname) in &connected_aps {
            println!("  - {} (ID: {})", hostname, id);
        }
        println!();

        let device_ids: Vec<i64> = connected_aps.iter().map(|(id, _)| *id).collect();

        println!("Sending command '{}' to all connected APs...\n", command);

        let results = self.send_cli_command(&device_ids, command).await?;

        // Create a map of device_id -> hostname for output
        let hostname_map: std::collections::HashMap<i64, String> = connected_aps.into_iter().collect();

        // Open bssids.txt for writing - will contain normalized BSSIDs
        let mut bssid_file = File::create("bssids.txt")
            .context("Failed to create bssids.txt")?;

        // Open wifi-bssids.txt for writing - will contain only access mode interfaces
        let mut wifi_bssid_file = File::create("wifi-bssids.txt")
            .context("Failed to create wifi-bssids.txt")?;

        // Open wifi-bssids.csv for writing - CSV format of access mode interfaces
        let mut wifi_bssid_csv = File::create("wifi-bssids.csv")
            .context("Failed to create wifi-bssids.csv")?;

        // Write header for wifi-bssids.txt once at the top
        writeln!(wifi_bssid_file, "{:<20} {:<20} {:<12} {:<20} {:<8} {:<8} {:<12} {:<6} {:<12} {:<12} {}",
            "Device", "DeviceID", "Name", "MAC", "Mode", "State", "Channel", "VLAN", "Radio", "Hive", "SSID")
            .context("Failed to write column header to wifi-bssids.txt")?;
        writeln!(wifi_bssid_file, "{}", "-".repeat(140))
            .context("Failed to write separator to wifi-bssids.txt")?;

        // Write CSV header
        writeln!(wifi_bssid_csv, "Device,DeviceID,Name,MAC,Mode,State,Channel,VLAN,Radio,Hive,SSID")
            .context("Failed to write CSV header to wifi-bssids.csv")?;

        // Build JSON output for saving to file
        let mut json_results = Vec::new();
        let mut total_bssids = 0;
        let mut total_wifi_bssids = 0;

        println!("=== CLI Command Results ===\n");
        for (device_id, output) in &results {
            let hostname = hostname_map.get(device_id).map(|s| s.as_str()).unwrap_or("unknown");

            // Extract and normalize interface entries using the parser module
            let interfaces = extract_interfaces(output);
            if !interfaces.is_empty() {
                println!("  {} (ID: {}): Found {} interface(s)", hostname, device_id, interfaces.len());
                total_bssids += interfaces.len();

                // Write full interface data to file with device context
                writeln!(bssid_file, "--- {} (ID: {}) ---", hostname, device_id)
                    .context("Failed to write header to bssids.txt")?;
                writeln!(bssid_file, "{:<12} {:<20} {:<8} {:<8} {:<12} {:<6} {:<8} {:<12} {}",
                    "Name", "MAC", "Mode", "State", "Channel", "VLAN", "Radio", "Hive", "SSID")
                    .context("Failed to write column header to bssids.txt")?;
                writeln!(bssid_file, "{}", "-".repeat(100))
                    .context("Failed to write separator to bssids.txt")?;
                for iface in &interfaces {
                    writeln!(bssid_file, "{:<12} {:<20} {:<8} {:<8} {:<12} {:<6} {:<8} {:<12} {}",
                        iface.name, iface.mac, iface.mode, iface.state,
                        iface.channel, iface.vlan, iface.radio, iface.hive, iface.ssid)
                        .context("Failed to write interface to bssids.txt")?;
                }
                writeln!(bssid_file).context("Failed to write newline to bssids.txt")?;

                // Filter and write access-mode interfaces to wifi-bssids.txt
                let access_interfaces: Vec<_> = interfaces.iter()
                    .filter(|iface| iface.mode.to_lowercase() == "access")
                    .collect();

                if !access_interfaces.is_empty() {
                    total_wifi_bssids += access_interfaces.len();
                    for iface in &access_interfaces {
                        // Write to txt file (fixed-width format)
                        writeln!(wifi_bssid_file, "{:<20} {:<20} {:<12} {:<20} {:<8} {:<8} {:<12} {:<6} {:<12} {:<12} {}",
                            hostname, device_id, iface.name, iface.mac, iface.mode, iface.state,
                            iface.channel, iface.vlan, iface.radio, iface.hive, iface.ssid)
                            .context("Failed to write interface to wifi-bssids.txt")?;

                        // Write to CSV file (with proper escaping)
                        writeln!(wifi_bssid_csv, "{},{},{},{},{},{},{},{},{},{},{}",
                            csv_escape(hostname),
                            device_id,
                            csv_escape(&iface.name),
                            csv_escape(&iface.mac),
                            csv_escape(&iface.mode),
                            csv_escape(&iface.state),
                            csv_escape(&iface.channel),
                            csv_escape(&iface.vlan),
                            csv_escape(&iface.radio),
                            csv_escape(&iface.hive),
                            csv_escape(&iface.ssid))
                            .context("Failed to write interface to wifi-bssids.csv")?;
                    }
                }
            }

            json_results.push(serde_json::json!({
                "device_id": device_id,
                "hostname": hostname,
                "command": command,
                "output": output
            }));
        }

        // Save to full_cli.json
        let json_data = serde_json::to_string_pretty(&json_results)
            .context("Failed to serialize CLI results to JSON")?;

        let mut file = File::create("full_cli.json")
            .context("Failed to create full_cli.json")?;

        file.write_all(json_data.as_bytes())
            .context("Failed to write CLI results to file")?;

        println!("CLI results saved to full_cli.json");
        println!("CLI output saved to bssids.txt ({} BSSIDs found)", total_bssids);
        println!("Access mode BSSIDs saved to wifi-bssids.txt ({} entries)", total_wifi_bssids);
        println!("Access mode BSSIDs saved to wifi-bssids.csv ({} entries)", total_wifi_bssids);

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    println!("Developed by Jeff Buddington www.linkedin.com/in/jeff-buddington-5178ba4");
    println!();

    let args: Vec<String> = env::args().collect();

    let base_url = env::var("XIQ_BASE_URL")
        .unwrap_or_else(|_| "https://api.extremecloudiq.com".to_string());

    let username = env::var("XIQ_USERNAME")
        .context("XIQ_USERNAME environment variable not set")?;

    let password = env::var("XIQ_PASSWORD")
        .context("XIQ_PASSWORD environment variable not set")?;

    let mut client = CloudIQClient::new(base_url);

    println!("Authenticating with Extreme CloudIQ...");
    client.login(&username, &password).await?;

    // Determine the CLI command to run
    let command = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        "show interface".to_string()
    };

    // Save devices to file and database
    println!("Fetching devices...");
    client.save_devices_to_file("devices.json").await?;

    println!("Connecting to database...");
    let db = Database::new("xiq-db").await?;

    println!("Saving devices to database...");
    client.save_devices_to_db(&db).await?;

    let count = db.count_devices().await?;
    println!("Database now contains {} devices", count);

    // Run CLI command on connected APs
    println!("\nRunning CLI command on connected APs...");
    client.run_command_on_connected_aps(&command).await?;

    println!("\nDone!");

    Ok(())
}
