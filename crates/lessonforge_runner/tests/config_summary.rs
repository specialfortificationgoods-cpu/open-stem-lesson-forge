use std::error::Error;

use lessonforge_runner::{
    RunnerMode, automated_repair_attempt_bucket, capability_summary, validate_runner_config,
};

#[test]
fn dummy_planner_config_validates_and_emits_redacted_summary() -> Result<(), Box<dyn Error>> {
    let config = dummy_planner_config();

    let validated = validate_runner_config(&config)?;
    let summary = capability_summary(&validated)?;
    let rendered = format!("{summary:?}");

    assert_eq!(summary.runner_id, "actor_planner_001");
    assert_eq!(summary.capabilities.phases, vec!["request_planning"]);
    assert_eq!(summary.capabilities.task_types, vec!["propose_task_graph"]);
    assert_eq!(
        summary.capabilities.workflow_capabilities,
        vec![
            "request_interpretation",
            "task_decomposition",
            "policy_reasoning"
        ]
    );
    assert_eq!(summary.capabilities.tools, vec!["structured_json_output"]);
    assert!(!summary.capabilities.automated_repair_loop_opt_in);
    assert_eq!(summary.capabilities.automated_repair_attempt_bucket, "0");
    assert_eq!(summary.trust_level, "runner_candidate");
    assert!(!rendered.contains("LESSONFORGE_RUNNER_TOKEN"));
    assert!(!rendered.contains(".lessonforge-runner-work"));
    assert!(!rendered.contains("127.0.0.1"));
    assert!(!rendered.contains("pinned_ca_pem_path"));
    Ok(())
}

#[test]
fn unsafe_origins_and_provider_backed_modes_are_rejected_safely() {
    for config in [
        dummy_planner_config_with_base("https://"),
        dummy_planner_config_with_base("https://*.lessonforge.example"),
        dummy_planner_config_with_base("https://attacker.example"),
        dummy_planner_config_with_base("http://lessonforge.example"),
        dummy_planner_config_with_base("https://user:pass@lessonforge.example"),
        dummy_planner_config_with_base("https://lessonforge.example/api"),
        dummy_planner_config_with_base("https://ollama.local:11434"),
        dummy_planner_config_with_base("http://127.0.0.1:11434"),
        dummy_planner_config_with_base("http://[::1]:11434"),
        dummy_planner_config_with_base("http://127.0.0.1:011434"),
        dummy_planner_config_with_base("http://[::1]:011434"),
    ] {
        let (code, rendered) = config_error_code_and_debug(&config);
        assert_eq!(code, "unsafe_central_api_origin");
        assert!(!rendered.contains("user:pass"));
    }

    let mut provider = dummy_planner_config();
    provider.policy.allow_provider_backed_modes = true;
    assert_eq!(
        config_error_code(&provider),
        "provider_backed_modes_unavailable"
    );

    let mut tool_execution = dummy_planner_config();
    tool_execution.policy.allow_tool_execution = true;
    assert_eq!(
        config_error_code(&tool_execution),
        "tool_execution_unavailable"
    );
}

#[test]
fn non_loopback_https_origins_require_complete_pinned_transport() -> Result<(), Box<dyn Error>> {
    let ca_dir = tempfile::tempdir()?;
    let ca_path = ca_dir.path().join("runner-ca.pem");
    std::fs::write(&ca_path, "fixture ca")?;

    let mut empty_pin = dummy_planner_config_with_base("https://lessonforge.example");
    empty_pin.transport.tls.trust_policy = lessonforge_runner::TlsTrustPolicy::PinnedCa;
    assert_eq!(config_error_code(&empty_pin), "unsafe_central_api_origin");

    let mut loopback_pinned_ca = dummy_planner_config_with_base("http://127.0.0.1:8080");
    loopback_pinned_ca.transport.tls.trust_policy = lessonforge_runner::TlsTrustPolicy::PinnedCa;
    assert_eq!(
        config_error_code(&loopback_pinned_ca),
        "unsafe_central_api_origin"
    );
    loopback_pinned_ca.transport.tls.pinned_ca_pem_path = ca_path.display().to_string();
    loopback_pinned_ca.transport.tls.expected_server_name = "127.0.0.1".to_owned();
    assert_eq!(
        config_error_code(&loopback_pinned_ca),
        "unsafe_central_api_origin"
    );

    let mut loopback_pinned_spki = dummy_planner_config_with_base("http://127.0.0.1:8080");
    loopback_pinned_spki.transport.tls.trust_policy =
        lessonforge_runner::TlsTrustPolicy::PinnedSpki;
    assert_eq!(
        config_error_code(&loopback_pinned_spki),
        "unsafe_central_api_origin"
    );
    loopback_pinned_spki.transport.tls.pinned_spki_sha256 =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_owned();
    loopback_pinned_spki.transport.tls.expected_server_name = "127.0.0.1".to_owned();
    assert_eq!(
        config_error_code(&loopback_pinned_spki),
        "unsafe_central_api_origin"
    );

    let mut wrong_name = empty_pin.clone();
    wrong_name.transport.tls.pinned_ca_pem_path = ca_path.display().to_string();
    wrong_name.transport.tls.expected_server_name = "other.example".to_owned();
    assert_eq!(config_error_code(&wrong_name), "unsafe_central_api_origin");

    let mut unreadable_pin = empty_pin.clone();
    unreadable_pin.transport.tls.pinned_ca_pem_path =
        ca_dir.path().join("missing-ca.pem").display().to_string();
    unreadable_pin.transport.tls.expected_server_name = "lessonforge.example".to_owned();
    assert_eq!(
        config_error_code(&unreadable_pin),
        "unsafe_central_api_origin"
    );

    let mut pinned_ca = empty_pin;
    pinned_ca.transport.tls.pinned_ca_pem_path = ca_path.display().to_string();
    pinned_ca.transport.tls.expected_server_name = "lessonforge.example".to_owned();
    validate_runner_config(&pinned_ca)?;

    let mut pinned_spki = dummy_planner_config_with_base("https://lessonforge.example");
    pinned_spki.transport.tls.trust_policy = lessonforge_runner::TlsTrustPolicy::PinnedSpki;
    pinned_spki.transport.tls.pinned_spki_sha256 =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_owned();
    pinned_spki.transport.tls.expected_server_name = "lessonforge.example".to_owned();
    validate_runner_config(&pinned_spki)?;
    Ok(())
}

#[test]
fn config_with_private_provider_or_path_fields_is_rejected_or_kept_out_of_summary() {
    let mut config = dummy_planner_config();
    config.forbidden.provider_api_key_literal = Some("sk-secret".to_owned());
    assert_eq!(config_error_code(&config), "unsafe_private_config_field");

    let mut prompt_path = dummy_planner_config();
    prompt_path.forbidden.local_prompt_template = Some("/Users/alice/prompts/lesson.md".to_owned());
    assert_eq!(
        config_error_code(&prompt_path),
        "unsafe_private_config_field"
    );

    let mut public_name = dummy_planner_config();
    public_name.runner.public_name = "/Users/alice/.ssh/id_rsa".to_owned();
    assert_eq!(config_error_code(&public_name), "unsafe_summary_field");

    let mut tool_name = dummy_planner_config();
    tool_name
        .capabilities
        .tools
        .push("https://api.openai.com".to_owned());
    assert_eq!(config_error_code(&tool_name), "unsafe_summary_field");
}

#[test]
fn automated_repair_loop_opt_in_is_allowed_only_for_auto_loop_mode() -> Result<(), Box<dyn Error>> {
    let mut invalid = dummy_planner_config();
    invalid.policy.automated_repair_loop_opt_in = true;
    invalid.policy.max_automated_repair_attempts = 1;
    assert_eq!(
        config_error_code(&invalid),
        "invalid_automated_repair_policy"
    );

    let mut repair = dummy_planner_config();
    repair.runner.runner_id = "runner_dummy_repair_auto_001".to_owned();
    repair.runner.public_name = "Dummy Repair Auto Loop".to_owned();
    repair.runner.mode = RunnerMode::DummyCodeRepairerAutoLoop;
    repair.capabilities = lessonforge_runner::CapabilityConfig::for_mode_for_test(
        RunnerMode::DummyCodeRepairerAutoLoop,
    );
    repair.policy.automated_repair_loop_opt_in = true;
    repair.policy.max_automated_repair_attempts = 2;
    let validated = validate_runner_config(&repair)?;
    let summary = capability_summary(&validated)?;
    assert!(summary.capabilities.automated_repair_loop_opt_in);
    assert_eq!(summary.capabilities.automated_repair_attempt_bucket, "2");
    assert_eq!(summary.capabilities.phases, vec!["code_repair"]);
    assert_eq!(
        summary.capabilities.task_types,
        vec!["repair_generated_code"]
    );
    assert_eq!(
        summary.capabilities.workflow_capabilities,
        vec![
            "artifact_generation",
            "basic_python",
            "python_execution_limited"
        ]
    );
    assert_eq!(automated_repair_attempt_bucket(2), Some("2"));
    assert_eq!(automated_repair_attempt_bucket(3), None);
    Ok(())
}

#[test]
fn mode_capabilities_must_match_canonical_dummy_mode() {
    let mut spoofed = dummy_planner_config();
    spoofed.capabilities.phases = vec!["code_repair".to_owned()];
    spoofed.capabilities.task_types = vec!["repair_generated_code".to_owned()];
    spoofed.capabilities.tools = vec![
        "structured_json_output".to_owned(),
        "sandboxed_python_checker_repair".to_owned(),
    ];
    assert_eq!(config_error_code(&spoofed), "capability_mode_mismatch");
}

fn config_error_code(config: &lessonforge_runner::RunnerConfig) -> &'static str {
    config_error_code_and_debug(config).0
}

fn config_error_code_and_debug(
    config: &lessonforge_runner::RunnerConfig,
) -> (&'static str, String) {
    match validate_runner_config(config) {
        Ok(_) => ("unexpected_ok", String::new()),
        Err(error) => (error.safe_code(), format!("{error:?}")),
    }
}

fn dummy_planner_config() -> lessonforge_runner::RunnerConfig {
    dummy_planner_config_with_base("https://127.0.0.1:8443")
}

fn dummy_planner_config_with_base(base: &str) -> lessonforge_runner::RunnerConfig {
    lessonforge_runner::RunnerConfig::dummy(
        "actor_planner_001",
        "Dummy Planner",
        RunnerMode::DummyPlanner,
        base,
    )
}
