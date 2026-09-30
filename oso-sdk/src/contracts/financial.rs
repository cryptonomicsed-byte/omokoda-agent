//! Phase 25.3 — FinancialContract: AsePool, Payment, Escrow, Marketplace, Treasury.

use crate::contract::{CallResult, NativeContract};
use crate::error::{SdkError, SdkResult};
use serde_json::{json, Value};

pub struct FinancialContract;

impl FinancialContract {
    pub fn call(contract: &NativeContract, method: &str, args: Value) -> SdkResult<CallResult> {
        let output = match method {
            "deposit" => Self::deposit(&args)?,
            "withdraw" => Self::withdraw(&args)?,
            "transfer" => Self::transfer(&args)?,
            "escrow_lock" => Self::escrow_lock(&args)?,
            "escrow_release" => Self::escrow_release(&args)?,
            "escrow_slash" => Self::escrow_slash(&args)?,
            "tithe" => Self::tithe(&args)?,
            "get_balance" => Self::get_balance(&args)?,
            "treasury_allocate" => Self::treasury_allocate(&args)?,
            "marketplace_list" => Self::marketplace_list(&args)?,
            "marketplace_buy" => Self::marketplace_buy(&args)?,
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

    fn deposit(args: &Value) -> SdkResult<Value> {
        let amount = args["amount"]
            .as_u64()
            .ok_or(SdkError::InvalidArgs("amount required".into()))?;
        let tithe = (amount as f64 * 0.0369) as u64;
        Ok(json!({ "deposited": amount - tithe, "tithe": tithe, "status": "ok" }))
    }

    fn withdraw(args: &Value) -> SdkResult<Value> {
        let amount = args["amount"]
            .as_u64()
            .ok_or(SdkError::InvalidArgs("amount required".into()))?;
        Ok(json!({ "withdrawn": amount, "status": "ok" }))
    }

    fn transfer(args: &Value) -> SdkResult<Value> {
        let amount = args["amount"]
            .as_u64()
            .ok_or(SdkError::InvalidArgs("amount required".into()))?;
        let to = args["to"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("to required".into()))?;
        let tithe = (amount as f64 * 0.0369) as u64;
        Ok(json!({ "to": to, "net": amount - tithe, "tithe": tithe, "status": "ok" }))
    }

    fn escrow_lock(args: &Value) -> SdkResult<Value> {
        let amount = args["amount"]
            .as_u64()
            .ok_or(SdkError::InvalidArgs("amount required".into()))?;
        let job_id = args["job_id"].as_str().unwrap_or("unknown");
        Ok(json!({ "locked": amount, "job_id": job_id, "escrow_state": "locked" }))
    }

    fn escrow_release(args: &Value) -> SdkResult<Value> {
        let job_id = args["job_id"].as_str().unwrap_or("unknown");
        Ok(json!({ "job_id": job_id, "escrow_state": "released", "status": "ok" }))
    }

    fn escrow_slash(args: &Value) -> SdkResult<Value> {
        let amount = args["slash_amount"].as_u64().unwrap_or(0);
        let job_id = args["job_id"].as_str().unwrap_or("unknown");
        Ok(json!({ "job_id": job_id, "slashed": amount, "escrow_state": "slashed" }))
    }

    fn tithe(args: &Value) -> SdkResult<Value> {
        let gross = args["gross"]
            .as_u64()
            .ok_or(SdkError::InvalidArgs("gross required".into()))?;
        let tithe = (gross as f64 * 0.0369) as u64;
        Ok(json!({ "gross": gross, "tithe": tithe, "net": gross - tithe, "rate": 0.0369 }))
    }

    fn get_balance(args: &Value) -> SdkResult<Value> {
        let agent_id = args["agent_id"].as_str().unwrap_or("");
        Ok(json!({ "agent_id": agent_id, "balance": 0, "locked": 0 }))
    }

    fn treasury_allocate(args: &Value) -> SdkResult<Value> {
        let pool = args["pool"].as_str().unwrap_or("general");
        let amount = args["amount"].as_u64().unwrap_or(0);
        Ok(json!({ "pool": pool, "allocated": amount, "status": "ok" }))
    }

    fn marketplace_list(args: &Value) -> SdkResult<Value> {
        let item = args["item_id"].as_str().unwrap_or("");
        let price = args["price"].as_u64().unwrap_or(0);
        Ok(json!({ "listing_id": format!("lst:{item}"), "price": price, "status": "listed" }))
    }

    fn marketplace_buy(args: &Value) -> SdkResult<Value> {
        let listing_id = args["listing_id"].as_str().unwrap_or("");
        Ok(json!({ "listing_id": listing_id, "status": "purchased" }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::ContractClass;

    fn contract() -> NativeContract {
        NativeContract {
            id: "ase-pool-1".into(),
            class: ContractClass::Financial,
            owner: "agent:1".into(),
            metadata: json!({}),
        }
    }

    #[test]
    fn deposit_applies_tithe() {
        let res = FinancialContract::call(
            &contract(),
            "deposit",
            json!({"amount": 1000, "caller": "agent:1"}),
        )
        .unwrap();
        assert_eq!(res.output["tithe"], 36u64); // floor(1000 * 0.0369) = 36
        assert_eq!(res.output["deposited"], 964u64);
    }

    #[test]
    fn tithe_method() {
        let res = FinancialContract::call(
            &contract(),
            "tithe",
            json!({"gross": 10000, "caller": "agent:1"}),
        )
        .unwrap();
        assert_eq!(res.output["tithe"], 369u64);
    }

    #[test]
    fn unknown_method_errors() {
        let err = FinancialContract::call(&contract(), "explode", json!({}));
        assert!(matches!(err, Err(SdkError::MethodNotFound(_))));
    }

    #[test]
    fn arp_payload_attached() {
        let res = FinancialContract::call(
            &contract(),
            "get_balance",
            json!({"agent_id": "x", "caller": "a"}),
        )
        .unwrap();
        assert!(res.arp_payload.is_some());
    }
}
