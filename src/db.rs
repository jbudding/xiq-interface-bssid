use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(database_name: &str) -> Result<Self> {
        let database_url = format!("{}.db", database_name);

        let options = SqliteConnectOptions::from_str(&database_url)?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .context("Failed to connect to database")?;

        let db = Self { pool };
        db.create_table().await?;

        Ok(db)
    }

    async fn create_table(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS devices (
                id INTEGER PRIMARY KEY,
                config_mismatch BOOLEAN,
                connected BOOLEAN,
                description TEXT,
                device_admin_state TEXT,
                device_function TEXT,
                hostname TEXT,
                ip_address TEXT,
                mac_address TEXT,
                managed_by TEXT,
                org_id INTEGER,
                product_type TEXT,
                serial_number TEXT,
                simulated BOOLEAN,
                software_version TEXT,
                system_up_time INTEGER,
                fetched_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to create devices table")?;

        Ok(())
    }

    pub async fn clear_devices(&self) -> Result<()> {
        sqlx::query("DELETE FROM devices")
            .execute(&self.pool)
            .await
            .context("Failed to clear devices table")?;

        Ok(())
    }

    pub async fn insert_device(&self, device: &serde_json::Value) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO devices (
                id, config_mismatch, connected, description, device_admin_state,
                device_function, hostname, ip_address, mac_address, managed_by,
                org_id, product_type, serial_number, simulated, software_version,
                system_up_time
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(device.get("id").and_then(|v| v.as_i64()))
        .bind(device.get("config_mismatch").and_then(|v| v.as_bool()))
        .bind(device.get("connected").and_then(|v| v.as_bool()))
        .bind(device.get("description").and_then(|v| v.as_str()))
        .bind(device.get("device_admin_state").and_then(|v| v.as_str()))
        .bind(device.get("device_function").and_then(|v| v.as_str()))
        .bind(device.get("hostname").and_then(|v| v.as_str()))
        .bind(device.get("ip_address").and_then(|v| v.as_str()))
        .bind(device.get("mac_address").and_then(|v| v.as_str()))
        .bind(device.get("managed_by").and_then(|v| v.as_str()))
        .bind(device.get("org_id").and_then(|v| v.as_i64()))
        .bind(device.get("product_type").and_then(|v| v.as_str()))
        .bind(device.get("serial_number").and_then(|v| v.as_str()))
        .bind(device.get("simulated").and_then(|v| v.as_bool()))
        .bind(device.get("software_version").and_then(|v| v.as_str()))
        .bind(device.get("system_up_time").and_then(|v| v.as_i64()))
        .execute(&self.pool)
        .await
        .context("Failed to insert device")?;

        Ok(())
    }

    pub async fn insert_devices(&self, devices: &[serde_json::Value]) -> Result<()> {
        // Clear existing devices first
        self.clear_devices().await?;

        // Insert new devices
        for device in devices {
            self.insert_device(device).await?;
        }

        println!("Successfully saved {} devices to database", devices.len());

        Ok(())
    }

    pub async fn count_devices(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM devices")
            .fetch_one(&self.pool)
            .await
            .context("Failed to count devices")?;

        Ok(row.0)
    }
}
