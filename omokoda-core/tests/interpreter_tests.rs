#[cfg(test)]
mod interpreter_tests {
    use omokoda_core::interpreter::{Steward, TurnEvent};
    use omokoda_core::parser::parse;
    use omokoda_core::reputation::{tier_for, tool_allowed, tools_for_tier};

    macro_rules! test_steward {
        ($name:expr) => {{
            let mut path = std::env::current_dir().unwrap();
            path.push("target");
            path.push("test_sessions");
            path.push($name);
            if path.exists() {
                let _ = std::fs::remove_dir_all(&path);
            }
            std::fs::create_dir_all(&path).unwrap();
            Steward::new().with_session_dir(path)
        }};
    }

    #[tokio::test]
    async fn birth_creates_agent_at_tier_zero() {
        let mut steward = test_steward!("birth_creates_agent_at_tier_zero");
        let stmts = parse(r#"birth "luna""#).unwrap();
        steward.dispatch(stmts[0].clone()).await.unwrap();
        assert_eq!(steward.reputation(), 0.000);
        assert_eq!(steward.tier(), 0);
    }

    #[tokio::test]
    async fn birth_initializes_structured_agent_core() {
        let mut steward = test_steward!("birth_initializes_structured_agent_state");
        let stmts = parse(r#"birth "luna""#).unwrap();
        steward.dispatch(stmts[0].clone()).await.unwrap();

        let agent = steward.agent_core().expect("agent exists after birth");
        assert!(agent.id().as_str().starts_with("agent-"));
        assert_eq!(agent.name(), "luna");
        assert!(agent.birth_timestamp() > 0);
        assert_eq!(agent.odu_seed().len(), 32);
        assert_eq!(agent.dna_fingerprint().len(), 86);
        assert!(!agent.odu_identity().mnemonic.is_empty());
        assert!(agent.odu_identity().mnemonic.split_whitespace().count() >= 1);
        assert_eq!(agent.reputation(), 0.0);
        assert_eq!(agent.tier(), 0);
    }

    #[tokio::test]
    async fn think_produces_receipt() {
        let mut steward = test_steward!("think_produces_receipt");
        steward.set_mock_provider("mock thought".to_string());
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);
        // Boost budget

        let stmts = parse(r#"think "hello world""#).unwrap();
        let result = steward.dispatch(stmts[0].clone()).await.unwrap();
        assert!(result.receipt.is_some());
        assert_eq!(result.receipt.unwrap().action, "think");
        assert_eq!(result.tool_output, Some("mock thought".to_string()));
    }

    #[tokio::test]
    async fn nominal_think_pays_metabolic_burn_without_bb_penalty() {
        // A short think on a fresh agent sits well inside its Busy Beaver
        // ceiling: the only synapse cost is the metabolic burn (1000 floor),
        // with neither the exceed penalty (-2500) nor the utilization bonus
        // (+1000) applied.
        let mut steward = test_steward!("nominal_think_pays_metabolic_burn_without_bb_penalty");
        steward.set_mock_provider("mock thought".to_string());
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);

        let stmts = parse(r#"think "hello world""#).unwrap();
        let result = steward.dispatch(stmts[0].clone()).await.unwrap();
        assert!(result.receipt.is_some());

        let synapse = steward.agent_core().unwrap().synapse();
        assert_eq!(
            synapse,
            99_000.0,
            "expected exactly the 1000-synapse metabolic burn, got a delta of {}",
            100_000.0 - synapse
        );
    }

    #[tokio::test]
    async fn think_dispatch_with_event_sink_emits_cycle_events() {
        use tokio::sync::mpsc;

        let mut steward = test_steward!("think_dispatch_with_event_sink_emits_cycle_events");
        steward.set_mock_provider("mock thought".to_string());
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);
        // Boost budget

        let (tx, mut rx) = mpsc::channel(10);
        let stmt = parse(r#"think "hello world""#).unwrap()[0].clone();
        let result = steward.dispatch_with_event_sink(stmt, tx).await.unwrap();

        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        assert!(result.receipt.is_some());
        assert!(events.iter().any(|e| matches!(e, TurnEvent::Started)));
        assert!(events
            .iter()
            .any(|e| matches!(e, TurnEvent::IntentCompiled(_))));
        assert!(events
            .iter()
            .any(|e| matches!(e, TurnEvent::PlanGenerated(_))));
        assert!(events
            .iter()
            .any(|e| matches!(e, TurnEvent::ReceiptGenerated(_))));
        assert!(events.iter().any(|e| matches!(e, TurnEvent::Finished)));
    }

    #[tokio::test]
    async fn act_dispatch_with_event_sink_emits_receipt_event() {
        use tokio::sync::mpsc;

        let mut steward = test_steward!("act_dispatch_with_event_sink_emits_receipt_event");
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);
        // Boost budget
        let test_file = "test_act_event.txt";
        std::fs::write(test_file, "content").unwrap();

        let (tx, mut rx) = mpsc::channel(10);
        let stmt = parse(r#"act "read_file" "test_act_event.txt""#).unwrap()[0].clone();
        let result = steward.dispatch_with_event_sink(stmt, tx).await.unwrap();

        let mut has_receipt = false;
        while let Ok(event) = rx.try_recv() {
            if let TurnEvent::ReceiptGenerated(_) = event {
                has_receipt = true;
            }
        }

        let _ = std::fs::remove_file(test_file);
        assert!(result.receipt.is_some());
        assert!(has_receipt);
    }

    #[tokio::test]
    async fn think_private_sets_private_mode() {
        let mut steward = test_steward!("think_private_sets_private_mode");
        steward.set_mock_provider("mock thought".to_string());
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);
        // Boost budget

        let stmts = parse(r#"think "secret""#).unwrap();
        let result = steward.dispatch(stmts[0].clone()).await.unwrap();
        assert!(result.private_mode);
    }

    #[tokio::test]
    async fn think_publish_sets_public_mode() {
        let mut steward = test_steward!("think_publish_sets_public_mode");
        steward.set_mock_provider("mock thought".to_string());
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);
        // Boost budget

        let stmts = parse(r#"think "share this" /publish"#).unwrap();
        let result = steward.dispatch(stmts[0].clone()).await.unwrap();
        assert!(!result.private_mode);
    }

    /// The causal DAG and reflection ledger sit on the same AgentSnapshot as
    /// odu_dir (so all three persist to the same disk file) and must
    /// observe the identical !private gate -- proves a private think does
    /// NOT leave a trace in either, the same invariant odu_dir already
    /// enforces to avoid a plaintext leak on save.
    #[tokio::test]
    async fn private_think_leaves_no_causal_or_reflection_trace() {
        let mut steward = test_steward!("private_think_leaves_no_causal_or_reflection_trace");
        steward.set_mock_provider("mock thought".to_string());
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);

        let stmts = parse(r#"think "secret plan""#).unwrap();
        let result = steward.dispatch(stmts[0].clone()).await.unwrap();
        assert!(result.private_mode);

        let agent = steward.agent_core().expect("agent exists");
        assert_eq!(agent.snapshot.odu_dir.len(), 0, "no plaintext in odu_dir");
        assert_eq!(
            agent.snapshot.causal_dag.len(),
            0,
            "no causal node for a private think"
        );
        assert!(
            agent.snapshot.reflection.entries.is_empty(),
            "no reflection entry for a private think"
        );
    }

    /// The public counterpart: a published think DOES leave a causal node
    /// and a reflection entry, both wired at the same site as odu_dir.
    #[tokio::test]
    async fn public_think_records_a_causal_node_and_a_reflection() {
        let mut steward = test_steward!("public_think_records_a_causal_node_and_a_reflection");
        steward.set_mock_provider("mock thought".to_string());
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);

        let stmts = parse(r#"think "share this" /publish"#).unwrap();
        let result = steward.dispatch(stmts[0].clone()).await.unwrap();
        assert!(!result.private_mode);

        let agent = steward.agent_core().expect("agent exists");
        assert_eq!(agent.snapshot.odu_dir.len(), 1);
        assert_eq!(agent.snapshot.causal_dag.len(), 1);
        assert_eq!(agent.snapshot.reflection.entries.len(), 1);
        let reflection = &agent.snapshot.reflection.entries[0];
        assert_eq!(reflection.primitive, "think");
        assert!(reflection.emotion.is_some(), "emotion tag is populated");
    }

    #[tokio::test]
    async fn steward_can_register_a_provider() {
        use omokoda_core::providers::MockProvider;

        let mut steward = test_steward!("steward_can_register_a_provider");
        steward.register_provider(Box::new(MockProvider::new("mock thought".to_string())));
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);
        // Boost budget
        steward
            .dispatch(parse(r#"/configure provider:ollama"#).unwrap()[0].clone())
            .await
            .unwrap();

        let stmts = parse(r#"think "hello""#).unwrap();
        let result = steward.dispatch(stmts[0].clone()).await.unwrap();
        assert_eq!(result.tool_output, Some("mock thought".to_string()));
    }

    #[tokio::test]
    async fn configure_rejects_unknown_provider() {
        let mut steward = test_steward!("configure_rejects_unknown_provider");
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);
        // Boost budget

        let result = steward
            .dispatch(parse(r#"/configure provider:unknown"#).unwrap()[0].clone())
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown provider"));
    }

    #[tokio::test]
    async fn act_produces_receipt() {
        let mut steward = test_steward!("act_produces_receipt");
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);
        // Boost budget

        let test_file = "test_act_receipt.txt";
        std::fs::write(test_file, "content").unwrap();

        let stmts = parse(r#"act "read_file" "test_act_receipt.txt""#).unwrap();
        let result = steward.dispatch(stmts[0].clone()).await.unwrap();

        std::fs::remove_file(test_file).unwrap();

        assert!(result.receipt.is_some());
        let receipt = result.receipt.unwrap();
        assert!(!receipt.receipt_id.is_empty());
    }

    #[tokio::test]
    async fn act_increases_reputation_via_dynamic_formula() {
        let mut steward = test_steward!("act_increases_reputation_via_dynamic_formula");
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);
        // Boost budget

        let test_file = "test_act_rep.txt";
        std::fs::write(test_file, "content").unwrap();

        let before = steward.reputation();
        let stmts = parse(r#"act "read_file" "test_act_rep.txt""#).unwrap();
        steward.dispatch(stmts[0].clone()).await.unwrap();
        let after = steward.reputation();

        std::fs::remove_file(test_file).unwrap();

        assert!(after > before);
        assert!(after - before < 0.1);
        assert!(after - before > 0.0);
    }

    #[tokio::test]
    async fn reputation_gain_decreases_as_rep_grows() {
        let mut steward = test_steward!("reputation_gain_decreases_as_rep_grows");
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);
        // Boost budget

        let test_file = "test_act_rep_grows.txt";
        std::fs::write(test_file, "content").unwrap();

        // Gain at low rep
        let stmts = parse(r#"act "read_file" "test_act_rep_grows.txt""#).unwrap();
        steward.dispatch(stmts[0].clone()).await.unwrap();
        let gain_low = steward.reputation();

        steward.clear_cooldowns();
        steward.set_reputation_for_test(50.0);
        let before_high = steward.reputation();
        let stmts2 = parse(r#"act "read_file" "test_act_rep_grows.txt""#).unwrap();
        steward.dispatch(stmts2[0].clone()).await.unwrap();
        let gain_high = steward.reputation() - before_high;

        std::fs::remove_file(test_file).unwrap();

        assert!(gain_low > gain_high);
    }

    #[tokio::test]
    async fn act_rejected_for_tool_above_current_tier() {
        let mut steward = test_steward!("act_rejected_for_tool_above_current_tier");
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);
        // Boost budget

        let stmts = parse(r#"act "agent_orchestration" "task""#).unwrap();
        let result = steward.dispatch(stmts[0].clone()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn reputation_decay_on_inactivity() {
        let mut steward = test_steward!("reputation_decay_on_inactivity");
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);
        // Boost budget

        steward.set_reputation_for_test(10.0);
        steward.apply_daily_decay(1); // 1 day inactive
        assert!(steward.reputation() < 10.0);
        // penalty = 0.008 (decay) + 0.010 (sandbox) = 0.018
        assert!((steward.reputation() - 9.982).abs() < 0.001);
    }

    #[tokio::test]
    async fn reputation_cannot_go_below_zero() {
        let mut steward = test_steward!("reputation_cannot_go_below_zero");
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);
        // Boost budget

        steward.set_reputation_for_test(0.001);
        steward.apply_daily_decay(100); // massive inactivity
        assert_eq!(steward.reputation(), 0.000);
    }

    #[tokio::test]
    async fn multi_statement_executes_in_order() {
        let mut steward = test_steward!("multi_statement_executes_in_order");
        steward.set_mock_provider("done".to_string());

        let test_file = "test_multi.txt";
        std::fs::write(test_file, "content").unwrap();

        let input = r#"birth "luna" provider:ollama
think "hello"
act "read_file" "test_multi.txt""#;
        let stmts = parse(input).unwrap();
        assert_eq!(stmts.len(), 3);
        for stmt in stmts {
            steward.dispatch(stmt).await.unwrap();
        }

        std::fs::remove_file(test_file).unwrap();
        assert!(steward.reputation() > 0.0);
    }

    #[tokio::test]
    async fn steward_state_persists_between_dispatches() {
        let mut steward = test_steward!("steward_state_persists_between_dispatches");
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);
        // Boost budget

        let test_file = "test_persist.txt";
        std::fs::write(test_file, "content").unwrap();

        steward
            .dispatch(parse(r#"act "read_file" "test_persist.txt""#).unwrap()[0].clone())
            .await
            .unwrap();
        let rep_after_first = steward.reputation();

        steward.clear_cooldowns();
        steward
            .dispatch(parse(r#"act "read_file" "test_persist.txt""#).unwrap()[0].clone())
            .await
            .unwrap();
        let rep_after_second = steward.reputation();

        std::fs::remove_file(test_file).unwrap();
        assert!(rep_after_second > rep_after_first);
    }

    #[test]
    fn reputation_tier_boundaries() {
        // Closed-lower bounds: [0,20)→T0, [20,40)→T1, [40,60)→T2,
        // [60,80)→T3, [80,100)→T4, [100,∞)→T5.
        // Matches Tier::from_reputation() in justice/tier.rs.
        assert_eq!(tier_for(0.000), 0);
        assert_eq!(tier_for(19.999), 0);
        assert_eq!(tier_for(20.000), 1); // boundary inclusive-lower
        assert_eq!(tier_for(20.001), 1);
        assert_eq!(tier_for(39.999), 1);
        assert_eq!(tier_for(40.000), 2); // boundary inclusive-lower
        assert_eq!(tier_for(40.001), 2);
        assert_eq!(tier_for(59.999), 2);
        assert_eq!(tier_for(60.000), 3); // boundary inclusive-lower
        assert_eq!(tier_for(60.001), 3);
        assert_eq!(tier_for(79.999), 3);
        assert_eq!(tier_for(80.000), 4); // boundary inclusive-lower
        assert_eq!(tier_for(80.001), 4);
        assert_eq!(tier_for(99.999), 4);
        assert_eq!(tier_for(100.000), 5);
    }

    #[tokio::test]
    async fn act_returns_tool_output() {
        let mut steward = test_steward!("act_returns_tool_output");
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);
        // Boost budget

        let test_file = "test_act_read.txt";
        std::fs::write(test_file, "real file content").unwrap();

        let stmts = parse(r#"act "read_file" "test_act_read.txt""#).unwrap();
        let result = steward.dispatch(stmts[0].clone()).await.unwrap();

        std::fs::remove_file(test_file).unwrap();

        assert!(result.tool_output.is_some());
        let output = result.tool_output.unwrap();
        assert!(output.contains("real file content"));
        assert!(output.contains("\"file\":"));
    }

    #[test]
    fn tier_tool_unlocks_are_cumulative() {
        let tier_0 = tools_for_tier(0);
        assert!(tier_0.contains(&"web_search".to_string()));
        assert!(tier_0.contains(&"read_file".to_string()));
        assert!(tier_0.contains(&"glob".to_string()));
        assert!(tier_0.contains(&"grep".to_string()));
        assert!(tool_allowed(1, "image_gen_basic"));
        assert!(!tool_allowed(1, "code_runner"));
        assert!(tool_allowed(2, "code_runner"));
        assert!(!tool_allowed(2, "data_analysis"));
        assert!(tool_allowed(3, "data_analysis"));
        assert!(tool_allowed(3, "api_connect"));
        assert!(!tool_allowed(3, "agent_orchestration"));
        assert!(tool_allowed(4, "agent_orchestration"));
        assert!(!tool_allowed(4, "self_modification"));
        assert!(tool_allowed(5, "self_modification"));
        assert!(tool_allowed(5, "multi_agent_fabric"));
    }

    #[tokio::test]
    async fn birth_with_metadata_configures_session() {
        let mut steward = test_steward!("birth_with_metadata_configures_session");
        steward.set_mock_provider("mock response".to_string());
        let stmts = parse(r#"birth "luna" provider:ollama privacy:false sandbox:false"#).unwrap();
        steward.dispatch(stmts[0].clone()).await.unwrap();

        let agent = steward.agent_core().unwrap();
        let config = &agent.session().config;
        assert_eq!(config.default_provider, "ollama");
        assert!(!config.default_privacy);
        assert!(!config.default_sandbox);
    }

    #[tokio::test]
    async fn slash_seal_and_unlock_preserves_private_memory() {
        let mut steward = test_steward!("slash_seal_and_unlock_preserves_private_memory");
        steward.set_mock_provider("mock thought".to_string());
        steward
            .dispatch(parse(r#"birth "luna""#).unwrap()[0].clone())
            .await
            .unwrap();
        steward.ensure_born_mut().unwrap().set_synapse(100_000.0);
        // Boost budget

        steward
            .dispatch(parse(r#"think "secret thought""#).unwrap()[0].clone())
            .await
            .unwrap();
        assert!(steward.agent_core().unwrap().private_data().is_some());

        steward
            .dispatch(parse(r#"/seal mypass"#).unwrap()[0].clone())
            .await
            .unwrap();
        assert!(steward.agent_core().unwrap().private_data().is_none());
        assert!(steward
            .agent_core()
            .unwrap()
            .session()
            .encrypted_private
            .is_some());

        steward
            .dispatch(parse(r#"/unlock mypass"#).unwrap()[0].clone())
            .await
            .unwrap();
        assert!(steward.agent_core().unwrap().private_data().is_some());
    }

    #[tokio::test]
    async fn test_sovereign_event_bus_emits_birth_event() {
        use omokoda_core::bus::events::sovereign_event;
        use omokoda_core::Statement;

        let mut steward = test_steward!("test_sovereign_event_bus_emits_birth_event");
        let mut receiver = steward.event_bus.subscribe();

        let stmt = Statement::Birth {
            name: "test-agent".to_string(),
            metadata: vec![],
        };

        steward.dispatch(stmt).await.unwrap();

        let event = receiver.recv().await.unwrap();
        if let Some(sovereign_event::Event::AgentBorn(born)) = event.event {
            assert_eq!(born.dna.len(), 86);
            assert!(!born.mnemonic.is_empty());
        } else {
            panic!("Expected AgentBorn event");
        }
    }
}
