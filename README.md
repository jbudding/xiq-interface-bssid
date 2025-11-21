# XIQ Interface BSSID Tool

A Rust CLI tool for extracting WiFi interface and BSSID information from Extreme CloudIQ (XIQ) managed access points. Compiles to a self-contained executable on both Windows and Linux with no runtime dependencies.

## Features

- Authenticates with Extreme CloudIQ API
- Fetches all managed devices in 100-device page segments until all connected APs are processed
- Stores device inventory in SQLite database
- Executes CLI commands on connected access points
- Parses interface output and extracts BSSID information
- Normalizes MAC addresses to consistent colon-separated format
- Exports data in multiple formats (TXT, CSV, JSON)

## Prerequisites

- Rust 1.70+ (uses 2021 edition)
- Extreme CloudIQ account with API access

## Installation

```bash
git clone https://github.com/jbudding/xiq-interface-bssid.git
cd xiq-interface-bssid
cargo build --release
```

## Configuration

Create a `.env` file in the project root:

```env
XIQ_USERNAME=your_username
XIQ_PASSWORD=your_password
XIQ_BASE_URL=https://api.extremecloudiq.com  # optional, this is the default
```

## Usage

### Default Command (show interface)

```bash
cargo run --release
```

### Custom CLI Command

```bash
cargo run --release -- "show interface wifi0"
```

## Output Files

The tool generates several output files:

| File | Description |
|------|-------------|
| `devices.json` | Full device inventory from CloudIQ API |
| `xiq-db.db` | SQLite database with device records |
| `full_cli.json` | Raw CLI command output from all APs |
| `bssids.txt` | All parsed interfaces grouped by device |
| `wifi-bssids.txt` | Access-mode interfaces only (fixed-width) |
| `wifi-bssids.csv` | Access-mode interfaces only (CSV format) |

## Sample Output

### Console Output

```
Authenticating with Extreme CloudIQ...
Successfully authenticated with CloudIQ API
Fetching devices...
Fetching page 1 with limit 100...
Retrieved 45 devices from page 1
Reached last page (received 45 devices, less than limit of 100)
Successfully retrieved 45 total devices across all pages
Devices saved to devices.json
Connecting to database...
Saving devices to database...

=== Device Import Summary ===
Total devices imported: 45
Devices with device_function 'AP': 42
============================

Database now contains 45 devices

Running CLI command on connected APs...
Fetching page 1 with limit 100...
Retrieved 45 devices from page 1
Successfully retrieved 45 total devices across all pages

=== Found 38 connected APs ===
  - AP-Building1-Floor2 (ID: 123456789)
  - AP-Building1-Floor3 (ID: 123456790)
  - AP-Building2-Lobby (ID: 123456791)
  ...

Sending command 'show interface' to all connected APs...

=== CLI Command Results ===

  AP-Building1-Floor2 (ID: 123456789): Found 8 interface(s)
  AP-Building1-Floor3 (ID: 123456790): Found 8 interface(s)
  AP-Building2-Lobby (ID: 123456791): Found 6 interface(s)
  ...

CLI results saved to full_cli.json
CLI output saved to bssids.txt (312 BSSIDs found)
Access mode BSSIDs saved to wifi-bssids.txt (186 entries)
Access mode BSSIDs saved to wifi-bssids.csv (186 entries)

Done!
```

### wifi-bssids.txt (Fixed-Width Format)

```
Device               DeviceID             Name         MAC                  Mode     State    Channel      VLAN   Radio        Hive         SSID
--------------------------------------------------------------------------------------------------------------------------------------------
AP-Building1-Floor2  123456789            wifi0.1      00:11:22:33:44:55    access   Up       36(80)       10     wifi0        MainHive     Corporate-WiFi
AP-Building1-Floor2  123456789            wifi0.2      00:11:22:33:44:56    access   Up       36(80)       20     wifi0        MainHive     Guest-WiFi
AP-Building1-Floor2  123456789            wifi1.1      00:11:22:33:44:60    access   Up       6(20)        10     wifi1        MainHive     Corporate-WiFi
AP-Building1-Floor3  123456790            wifi0.1      AA:BB:CC:DD:EE:01    access   Up       149(80)      10     wifi0        MainHive     Corporate-WiFi
```

### wifi-bssids.csv (CSV Format)

```csv
Device,DeviceID,Name,MAC,Mode,State,Channel,VLAN,Radio,Hive,SSID
AP-Building1-Floor2,123456789,wifi0.1,00:11:22:33:44:55,access,Up,36(80),10,wifi0,MainHive,Corporate-WiFi
AP-Building1-Floor2,123456789,wifi0.2,00:11:22:33:44:56,access,Up,36(80),20,wifi0,MainHive,Guest-WiFi
AP-Building1-Floor2,123456789,wifi1.1,00:11:22:33:44:60,access,Up,6(20),10,wifi1,MainHive,Corporate-WiFi
AP-Building1-Floor3,123456790,wifi0.1,AA:BB:CC:DD:EE:01,access,Up,149(80),10,wifi0,MainHive,Corporate-WiFi
```

### bssids.txt (Full Interface Dump)

```
--- AP-Building1-Floor2 (ID: 123456789) ---
Name         MAC                  Mode     State    Channel      VLAN   Radio    Hive         SSID
----------------------------------------------------------------------------------------------------
wifi0        00:11:22:33:44:50    AP       Up       36(80)       1      wifi0    MainHive     -
wifi0.1      00:11:22:33:44:55    access   Up       36(80)       10     wifi0    MainHive     Corporate-WiFi
wifi0.2      00:11:22:33:44:56    access   Up       36(80)       20     wifi0    MainHive     Guest-WiFi
wifi1        00:11:22:33:44:5F    AP       Up       6(20)        1      wifi1    MainHive     -
wifi1.1      00:11:22:33:44:60    access   Up       6(20)        10     wifi1    MainHive     Corporate-WiFi

--- AP-Building1-Floor3 (ID: 123456790) ---
Name         MAC                  Mode     State    Channel      VLAN   Radio    Hive         SSID
----------------------------------------------------------------------------------------------------
wifi0        AA:BB:CC:DD:EE:00    AP       Up       149(80)      1      wifi0    MainHive     -
wifi0.1      AA:BB:CC:DD:EE:01    access   Up       149(80)      10     wifi0    MainHive     Corporate-WiFi
```

## API Endpoints Used

- `POST /login` - Authenticates and retrieves access token
- `GET /devices` - Retrieves all devices (with pagination)
- `POST /devices/:cli` - Executes CLI commands on devices

## MAC Address Normalization

The tool normalizes MAC addresses from various formats to a consistent colon-separated uppercase format:

| Input Format | Output |
|-------------|--------|
| `0011.2233.4455` | `00:11:22:33:44:55` |
| `001122334455` | `00:11:22:33:44:55` |
| `00-11-22-33-44-55` | `00:11:22:33:44:55` |
| `00:11:22:33:44:55` | `00:11:22:33:44:55` |

## Dependencies

- `reqwest` - HTTP client with TLS support
- `serde` / `serde_json` - JSON serialization
- `tokio` - Async runtime
- `anyhow` - Error handling
- `dotenv` - Environment variable management
- `sqlx` - SQLite database access
- `regex` - Interface output parsing

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

**Attribution Required:** Any use, redistribution, or incorporation of this software must include credit to [Jeff Buddington](https://www.linkedin.com/in/jeff-buddington-5178ba4).
