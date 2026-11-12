#[cfg(test)]
mod tool_registry_tests {
    use omokoda_core::permissions::{PermissionMode, PermissionPolicy};
    use omokoda_core::tools::ToolRegistry;
    use std::fs;

    #[tokio::test]
    async fn read_file_tool_basic() {
        let registry = ToolRegistry::new();
        let policy = PermissionPolicy::default_steward_policy(PermissionMode::WorkspaceWrite);
        let test_file = "test_read_file.txt";
        fs::write(test_file, "hello world").unwrap();

        let ctx = omokoda_core::tools::ExecutionContext {
            agent_id: omokoda_core::identity::AgentId::from_str("agent-1"),
            name: "luna".to_string(),
            tier: 0,
            reputation: 0.0,
            odu_identity: omokoda_core::identity::odu::OduIdentity {
                primary_index: 0,
                mnemonic: "".into(),
            },
            workspace_root: std::env::current_dir().unwrap(),
            sandbox_mode: false,
        };

        let result = registry
            .execute("read_file", test_file, ctx, &policy, None)
            .await
            .unwrap()
            .0;
        assert!(result.contains("hello world"));
        assert!(result.contains("\"file\":"));

        fs::remove_file(test_file).unwrap();
    }

    #[tokio::test]
    async fn glob_tool_basic() {
        let registry = ToolRegistry::new();
        let policy = PermissionPolicy::default_steward_policy(PermissionMode::WorkspaceWrite);
        fs::create_dir_all("test_glob_dir").unwrap();
        fs::write("test_glob_dir/a.txt", "a").unwrap();
        fs::write("test_glob_dir/b.txt", "b").unwrap();

        let ctx = omokoda_core::tools::ExecutionContext {
            agent_id: omokoda_core::identity::AgentId::from_str("agent-1"),
            name: "luna".to_string(),
            tier: 0,
            reputation: 0.0,
            odu_identity: omokoda_core::identity::odu::OduIdentity {
                primary_index: 0,
                mnemonic: "".into(),
            },
            workspace_root: std::env::current_dir().unwrap(),
            sandbox_mode: false,
        };

        let result = registry
            .execute("glob", "test_glob_dir/*.txt", ctx, &policy, None)
            .await
            .unwrap()
            .0;
        assert!(result.contains("test_glob_dir/a.txt"));
        assert!(result.contains("test_glob_dir/b.txt"));
        assert!(result.contains("\"filenames\":"));

        fs::remove_dir_all("test_glob_dir").unwrap();
    }

    #[tokio::test]
    async fn grep_tool_basic() {
        let registry = ToolRegistry::new();
        let policy = PermissionPolicy::default_steward_policy(PermissionMode::WorkspaceWrite);
        // Unique temp dir prevents races with other parallel tests.
        let tmp = std::env::temp_dir().join(format!("omokoda_grep_{}", std::process::id()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(
            tmp.join("test_grep.txt"),
            "line 1\nline 2 with target\nline 3",
        )
        .unwrap();

        let grep_input = serde_json::json!({
            "pattern": "target",
            "path": tmp.to_str().unwrap(),
            "glob": "**/test_grep.txt",
            "output_mode": "content"
        });

        let ctx = omokoda_core::tools::ExecutionContext {
            agent_id: omokoda_core::identity::AgentId::from_str("agent-1"),
            name: "luna".to_string(),
            tier: 0,
            reputation: 0.0,
            odu_identity: omokoda_core::identity::odu::OduIdentity {
                primary_index: 0,
                mnemonic: "".into(),
            },
            workspace_root: tmp.clone(),
            sandbox_mode: false,
        };

        let result = registry
            .execute("grep", &grep_input.to_string(), ctx, &policy, None)
            .await
            .unwrap()
            .0;
        assert!(result.contains("line 2 with target"));
        assert!(result.contains("\"content\":"));

        fs::remove_dir_all(&tmp).unwrap();
    }

    #[tokio::test]
    async fn tools_enforce_tier_gates() {
        let registry = ToolRegistry::new();
        let policy = PermissionPolicy::default_steward_policy(PermissionMode::DangerFullAccess);
        let workspace = std::env::current_dir().unwrap();

        // bash requires Tier 2
        let ctx0 = omokoda_core::tools::ExecutionContext {
            agent_id: omokoda_core::identity::AgentId::from_str("agent-1"),
            name: "luna".to_string(),
            tier: 0,
            reputation: 0.0,
            odu_identity: omokoda_core::identity::odu::OduIdentity {
                primary_index: 0,
                mnemonic: "".into(),
            },
            workspace_root: workspace.clone(),
            sandbox_mode: false,
        };
        let result = registry.execute("bash", "ls", ctx0, &policy, None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires tier 2"));

        let ctx2 = omokoda_core::tools::ExecutionContext {
            agent_id: omokoda_core::identity::AgentId::from_str("agent-1"),
            name: "luna".to_string(),
            tier: 2,
            reputation: 0.0,
            odu_identity: omokoda_core::identity::odu::OduIdentity {
                primary_index: 0,
                mnemonic: "".into(),
            },
            workspace_root: workspace,
            sandbox_mode: false,
        };
        let result = registry.execute("bash", "ls", ctx2, &policy, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn tools_block_path_traversal() {
        let registry = ToolRegistry::new();
        let policy = PermissionPolicy::default_steward_policy(PermissionMode::WorkspaceWrite);
        let ctx = omokoda_core::tools::ExecutionContext {
            agent_id: omokoda_core::identity::AgentId::from_str("agent-1"),
            name: "luna".to_string(),
            tier: 0,
            reputation: 0.0,
            odu_identity: omokoda_core::identity::odu::OduIdentity {
                primary_index: 0,
                mnemonic: "".into(),
            },
            workspace_root: std::env::current_dir().unwrap(),
            sandbox_mode: false,
        };

        let result = registry
            .execute("read_file", "../secrets.txt", ctx.clone(), &policy, None)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Boundary Violation"));

        let result = registry
            .execute("glob", "../**/*", ctx.clone(), &policy, None)
            .await;
        assert!(result.is_err());

        let grep_input = serde_json::json!({
            "pattern": "secret",
            "path": "../file.txt"
        });

        let result = registry
            .execute("grep", &grep_input.to_string(), ctx, &policy, None)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Boundary Violation"));
    }

    #[tokio::test]
    async fn bash_sandbox_rejects_parent_traversal() {
        let registry = ToolRegistry::new();
        let policy = PermissionPolicy::default_steward_policy(PermissionMode::DangerFullAccess);
        let ctx = omokoda_core::tools::ExecutionContext {
            agent_id: omokoda_core::identity::AgentId::from_str("agent-1"),
            name: "luna".to_string(),
            tier: 2,
            reputation: 0.0,
            odu_identity: omokoda_core::identity::odu::OduIdentity {
                primary_index: 0,
                mnemonic: "".into(),
            },
            workspace_root: std::env::current_dir().unwrap(),
            sandbox_mode: true,
        };
        let result = registry
            .execute("bash", "cd ../ && ls", ctx, &policy, None)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must not contain '..'"));
    }

    #[tokio::test]
    async fn note_taking_tool_basic() {
        let registry = ToolRegistry::new();
        let context = omokoda_core::tools::ExecutionContext {
            agent_id: omokoda_core::identity::AgentId::from_str("test-agent-1234567890"),
            name: "test".to_string(),
            tier: 0,
            reputation: 0.0,
            odu_identity: omokoda_core::identity::odu::OduIdentity {
                primary_index: 0,
                mnemonic: "test".to_string(),
            },
            workspace_root: std::env::current_dir().unwrap(),
            sandbox_mode: false,
        };

        let params = r#"{"title": "test_note", "content": "hello world"}"#;
        let (output, _) = registry
            .execute(
                "note_taking",
                params,
                context,
                &PermissionPolicy::default_steward_policy(
                    omokoda_core::permissions::PermissionMode::DangerFullAccess,
                ),
                None,
            )
            .await
            .unwrap();

        assert!(output.contains("hello world"));

        let note_path = std::env::current_dir().unwrap().join("notes/test_note.md");
        assert!(note_path.exists());
        let content = std::fs::read_to_string(&note_path).unwrap();
        assert_eq!(content, "hello world");

        std::fs::remove_file(note_path).unwrap();
        let _ = std::fs::remove_dir(std::env::current_dir().unwrap().join("notes"));
    }
}
