//! Phase 25.6 — DeviceContract: DeviceRegistry, DeviceManifest, SensorPolicy.

use crate::contract::{CallResult, NativeContract};
use crate::error::{SdkError, SdkResult};
use serde_json::{json, Value};

pub struct DeviceContract;

impl DeviceContract {
    pub fn call(contract: &NativeContract, method: &str, args: Value) -> SdkResult<CallResult> {
        let output = match method {
            "register_device" => Self::register_device(&args)?,
            "update_manifest" => Self::update_manifest(&args)?,
            "deregister" => Self::deregister(&args)?,
            "set_sensor_policy" => Self::set_sensor_policy(&args)?,
            "get_sensor_policy" => Self::get_sensor_policy(&args)?,
            "heartbeat" => Self::heartbeat(&args)?,
            "get_device" => Self::get_device(&args)?,
            "list_devices" => Self::list_devices(&args)?,
            _ => return Err(SdkError::MethodNotFound(method.into())),
        };
        let arp = super::arp_payload(
            contract,
            method,
            args["caller"].as_str().unwrap_or(""),
            &output,
        );
        Ok(CallResult {
            contract_id: contract.id.clone(),
            method: method.into(),
            output,
            arp_payload: Some(arp),
        })
    }

    fn register_device(args: &Value) -> SdkResult<Value> {
        let device_id = args["device_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("device_id required".into()))?;
        let owner = args["owner"].as_str().unwrap_or("");
        let device_type = args["device_type"].as_str().unwrap_or("generic");
        Ok(json!({
            "device_id": device_id,
            "owner": owner,
            "device_type": device_type,
            "status": "registered",
            "registry_id": format!("dev:{device_id}")
        }))
    }

    fn update_manifest(args: &Value) -> SdkResult<Value> {
        let device_id = args["device_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("device_id required".into()))?;
        let firmware = args["firmware_version"].as_str().unwrap_or("unknown");
        let capabilities = args["capabilities"].as_array().cloned().unwrap_or_default();
        Ok(json!({
            "device_id": device_id,
            "firmware_version": firmware,
            "capabilities": capabilities,
            "manifest_hash": format!("mfst:{device_id}:{firmware}"),
            "status": "updated"
        }))
    }

    fn deregister(args: &Value) -> SdkResult<Value> {
        let device_id = args["device_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("device_id required".into()))?;
        Ok(json!({ "device_id": device_id, "status": "deregistered" }))
    }

    fn set_sensor_policy(args: &Value) -> SdkResult<Value> {
        let device_id = args["device_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("device_id required".into()))?;
        let policy = &args["policy"];
        Ok(json!({
            "device_id": device_id,
            "policy": policy,
            "policy_id": format!("pol:{device_id}"),
            "status": "set"
        }))
    }

    fn get_sensor_policy(args: &Value) -> SdkResult<Value> {
        let device_id = args["device_id"].as_str().unwrap_or("");
        Ok(json!({ "device_id": device_id, "policy": null }))
    }

    fn heartbeat(args: &Value) -> SdkResult<Value> {
        let device_id = args["device_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("device_id required".into()))?;
        let ts = args["ts"].as_u64().unwrap_or(0);
        Ok(json!({ "device_id": device_id, "ts": ts, "status": "alive" }))
    }

    fn get_device(args: &Value) -> SdkResult<Value> {
        let device_id = args["device_id"].as_str().unwrap_or("");
        Ok(json!({ "device_id": device_id, "found": false }))
    }

    fn list_devices(args: &Value) -> SdkResult<Value> {
        let owner = args["owner"].as_str().unwrap_or("");
        Ok(json!({ "owner": owner, "devices": [] }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::ContractClass;

    fn contract() -> NativeContract {
        NativeContract {
            id: "dev-reg-1".into(),
            class: ContractClass::Device,
            owner: "a".into(),
            metadata: json!({}),
        }
    }

    #[test]
    fn register_device_ok() {
        let r = DeviceContract::call(&contract(), "register_device",
            json!({"device_id": "m5stick-01", "owner": "agent:1", "device_type": "m5stack", "caller": "agent:1"})).unwrap();
        assert_eq!(r.output["status"], "registered");
    }
}
