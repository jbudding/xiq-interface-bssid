use regex::Regex;
use serde::{Deserialize, Serialize};

/// Normalize a MAC address to colon-separated format (xx:xx:xx:xx:xx:xx)
/// Handles various input formats:
/// - 0011.2233.4455 (Cisco style with dots)
/// - 001122334455 (no separators)
/// - 00-11-22-33-44-55 (dash separated)
/// - 00:11:22:33:44:55 (already colon separated)
fn normalize_mac(mac: &str) -> String {
    // Remove all separators (colons, dots, dashes)
    let hex_only: String = mac.chars().filter(|c| c.is_ascii_hexdigit()).collect();

    // If we don't have exactly 12 hex characters, return the original uppercase
    if hex_only.len() != 12 {
        return mac.to_uppercase();
    }

    // Format as colon-separated pairs
    let bytes: Vec<&str> = vec![
        &hex_only[0..2],
        &hex_only[2..4],
        &hex_only[4..6],
        &hex_only[6..8],
        &hex_only[8..10],
        &hex_only[10..12],
    ];

    bytes.join(":").to_uppercase()
}

/// Represents a parsed interface entry from CLI output
/// Equivalent to the TextFSM template fields in hiveos.template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceEntry {
    pub name: String,
    pub mac: String,
    pub mode: String,
    pub state: String,
    pub channel: String,
    pub vlan: String,
    pub radio: String,
    pub hive: String,
    pub ssid: String,
}

/// Parser for HiveOS-style interface output
/// Replaces the TextFSM Python template with native Rust parsing
pub struct InterfaceParser {
    line_regex: Regex,
}

impl InterfaceParser {
    pub fn new() -> Self {
        // Build regex from the TextFSM template patterns:
        // NAME: \S+
        // MAC: [a-fA-F0-9:\.]+
        // MODE: \S+
        // STATE: \w+
        // CHANNEL: \S+
        // VLAN: \S+
        // RADIO: \S+
        // HIVE: \S+
        // SSID: \S+
        let line_regex = Regex::new(
            r"^(\S+)\s+([a-fA-F0-9:\.]+)\s+(\S+)\s+(\w+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s*$"
        ).expect("Failed to compile interface regex");

        Self { line_regex }
    }

    /// Parse CLI output and extract interface entries
    pub fn parse(&self, output: &str) -> Vec<InterfaceEntry> {
        let mut entries = Vec::new();

        for line in output.lines() {
            // Skip header lines, separator lines, and empty lines
            if line.trim().is_empty()
                || line.starts_with("Name")
                || line.starts_with('-')
                || line.contains("MAC addr")
            {
                continue;
            }

            if let Some(caps) = self.line_regex.captures(line) {
                let entry = InterfaceEntry {
                    name: caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default(),
                    mac: caps.get(2).map(|m| normalize_mac(m.as_str())).unwrap_or_default(),
                    mode: caps.get(3).map(|m| m.as_str().to_string()).unwrap_or_default(),
                    state: caps.get(4).map(|m| m.as_str().to_string()).unwrap_or_default(),
                    channel: caps.get(5).map(|m| m.as_str().to_string()).unwrap_or_default(),
                    vlan: caps.get(6).map(|m| m.as_str().to_string()).unwrap_or_default(),
                    radio: caps.get(7).map(|m| m.as_str().to_string()).unwrap_or_default(),
                    hive: caps.get(8).map(|m| m.as_str().to_string()).unwrap_or_default(),
                    ssid: caps.get(9).map(|m| m.as_str().to_string()).unwrap_or_default(),
                };
                entries.push(entry);
            }
        }

        entries
    }

    /// Extract all MAC addresses (BSSIDs) from parsed entries
    #[allow(dead_code)]
    pub fn extract_macs(&self, entries: &[InterfaceEntry]) -> Vec<String> {
        entries.iter().map(|e| e.mac.clone()).collect()
    }
}

impl Default for InterfaceParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract BSSIDs from raw CLI output using multiple strategies
pub fn extract_bssids(output: &str) -> Vec<String> {
    // Use extract_interfaces and return just the MACs for backward compatibility
    extract_interfaces(output)
        .into_iter()
        .map(|e| e.mac)
        .collect()
}

/// Extract full interface entries from raw CLI output
pub fn extract_interfaces(output: &str) -> Vec<InterfaceEntry> {
    let mut entries = Vec::new();
    let mac_regex = Regex::new(
        r"([0-9a-fA-F]{2}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2})"
    ).expect("Failed to compile MAC regex");

    // Strategy 1: Try structured parsing with InterfaceParser
    let parser = InterfaceParser::new();
    let parsed = parser.parse(output);
    if !parsed.is_empty() {
        entries.extend(parsed);
    }

    // Strategy 2: Also extract any BSSID-labeled MAC addresses (as minimal entries)
    for line in output.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.contains("bssid") {
            for cap in mac_regex.captures_iter(line) {
                let mac = normalize_mac(&cap[1]);
                if !entries.iter().any(|e| e.mac == mac) {
                    entries.push(InterfaceEntry {
                        name: String::new(),
                        mac,
                        mode: String::new(),
                        state: String::new(),
                        channel: String::new(),
                        vlan: String::new(),
                        radio: String::new(),
                        hive: String::new(),
                        ssid: String::new(),
                    });
                }
            }
        }
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_interface_output() {
        let output = r#"
Name     MAC addr           Mode   State  Chan(Width) VLAN  Radio Hive SSID
------   ---------------    -----  -----  ----------- ----  ----- ---- ----
wifi0    00:11:22:33:44:55  AP     up     11(20)      1     wifi0 hive1 TestSSID
wifi1    AA:BB:CC:DD:EE:FF  AP     up     36(80)      10    wifi1 hive2 Corp
"#;

        let parser = InterfaceParser::new();
        let entries = parser.parse(output);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "wifi0");
        assert_eq!(entries[0].mac, "00:11:22:33:44:55");
        assert_eq!(entries[0].ssid, "TestSSID");
        assert_eq!(entries[1].name, "wifi1");
        assert_eq!(entries[1].mac, "AA:BB:CC:DD:EE:FF");
    }

    #[test]
    fn test_extract_bssids() {
        let output = "BSSID: 00:11:22:33:44:55\nSome other line\nbssid AA:BB:CC:DD:EE:FF";
        let bssids = extract_bssids(output);

        assert!(bssids.contains(&"00:11:22:33:44:55".to_string()));
        assert!(bssids.contains(&"AA:BB:CC:DD:EE:FF".to_string()));
    }
}
