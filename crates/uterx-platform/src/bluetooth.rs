//! Bluetooth abstraction for device scanning and communication.

use anyhow::Result;

/// A discovered Bluetooth device.
#[derive(Debug, Clone)]
pub struct BtDevice {
    pub name: Option<String>,
    pub address: String,
    pub rssi: Option<i16>,
    pub connected: bool,
}

/// Bluetooth manager trait for cross-platform support.
#[allow(async_fn_in_trait)]
pub trait BluetoothManager {
    /// Start scanning for nearby BLE devices.
    async fn start_scan(&mut self) -> Result<()>;
    /// Stop scanning.
    async fn stop_scan(&mut self) -> Result<()>;
    /// Get the list of discovered devices.
    async fn devices(&self) -> Result<Vec<BtDevice>>;
    /// Connect to a device by address.
    async fn connect(&mut self, address: &str) -> Result<()>;
    /// Disconnect from a device.
    async fn disconnect(&mut self, address: &str) -> Result<()>;
    /// Send data to a connected device.
    async fn send(&mut self, address: &str, data: &[u8]) -> Result<()>;
    /// Receive data from a connected device.
    async fn receive(&mut self, address: &str) -> Result<Vec<u8>>;
}

/// Placeholder implementation using btleplug.
pub struct BtleplugManager {
    // TODO: btleplug adapter and session state
}

impl BtleplugManager {
    pub fn new() -> Self {
        Self {}
    }
}

impl BluetoothManager for BtleplugManager {
    async fn start_scan(&mut self) -> Result<()> {
        // TODO: btleplug scan implementation
        tracing::info!("BLE scan started (placeholder)");
        Ok(())
    }

    async fn stop_scan(&mut self) -> Result<()> {
        Ok(())
    }

    async fn devices(&self) -> Result<Vec<BtDevice>> {
        Ok(Vec::new())
    }

    async fn connect(&mut self, _address: &str) -> Result<()> {
        anyhow::bail!("BLE connect not yet implemented")
    }

    async fn disconnect(&mut self, _address: &str) -> Result<()> {
        Ok(())
    }

    async fn send(&mut self, _address: &str, _data: &[u8]) -> Result<()> {
        anyhow::bail!("BLE send not yet implemented")
    }

    async fn receive(&mut self, _address: &str) -> Result<Vec<u8>> {
        anyhow::bail!("BLE receive not yet implemented")
    }
}
