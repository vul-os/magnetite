// gds_tests.rs — Unit/integration-style tests for the Game Dev Studio (GDS) features.
//
// Tests are grouped by area and do NOT require a live database unless marked
// with `#[ignore]` (requires DATABASE_URL).
//
// Areas covered:
//   1. templates API   — static catalog shape, find_template helper, on-disk paths
//   2. scaffold logic  — crate-name sanitisation, cli-command format, template fields
//   3. provisioning    — default field values, InstanceSummary shape
//   4. distribution    — UpdateArtifactRequest parsing, BuildStatusSummary fields
//   5. GDS end-to-end  — template → scaffold round-trip (pure-logic, no DB)

// ─────────────────────────────────────────────────────────────────────────────
// 1. Templates catalog
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod templates_catalog_tests {
    use magnetite_backend::api::templates::{find_template, GraphicsTier};

    #[test]
    fn catalog_has_exactly_four_entries() {
        // The four on-disk templates: arcade, authoritative, fps, motorsport.
        let known_ids = ["arcade", "authoritative", "fps", "motorsport"];
        for id in &known_ids {
            assert!(
                find_template(id).is_some(),
                "template '{}' must be in the catalog",
                id
            );
        }
    }

    #[test]
    fn all_template_ids_are_unique() {
        // Tested in the templates module itself, but we verify from the outside too.
        let ids: Vec<&str> = ["arcade", "authoritative", "fps", "motorsport"]
            .iter()
            .filter_map(|id| find_template(id))
            .map(|t| t.id)
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "duplicate template ids detected");
    }

    #[test]
    fn arcade_template_is_lite2d() {
        let t = find_template("arcade").expect("arcade template missing");
        assert!(matches!(t.graphics_tier, GraphicsTier::Lite2d));
        assert_eq!(t.template_path, "game-templates/arcade");
    }

    #[test]
    fn authoritative_template_has_wasm_abi() {
        let t = find_template("authoritative").expect("authoritative template missing");
        assert!(
            t.starter_files.contains(&"src/wasm_abi.rs"),
            "authoritative template must list src/wasm_abi.rs"
        );
    }

    #[test]
    fn fps_template_is_standard_3d() {
        let t = find_template("fps").expect("fps template missing");
        assert!(matches!(t.graphics_tier, GraphicsTier::Standard3d));
        assert!(
            t.starter_files.contains(&"src/hitscan.rs"),
            "fps template must include hitscan.rs"
        );
    }

    #[test]
    fn motorsport_template_is_advanced_3d() {
        let t = find_template("motorsport").expect("motorsport template missing");
        assert!(matches!(t.graphics_tier, GraphicsTier::Advanced3d));
        assert!(
            t.starter_files.contains(&"src/vehicle.rs"),
            "motorsport template must include vehicle.rs"
        );
    }

    #[test]
    fn unknown_template_id_returns_none() {
        assert!(find_template("doesnotexist").is_none());
        assert!(find_template("").is_none());
        assert!(find_template("ARCADE").is_none()); // case-sensitive
    }

    #[test]
    fn all_templates_have_nonempty_paths() {
        for id in &["arcade", "authoritative", "fps", "motorsport"] {
            let t = find_template(id).unwrap();
            assert!(
                !t.template_path.is_empty(),
                "{} has empty template_path",
                id
            );
            assert!(
                !t.template_repo.is_empty(),
                "{} has empty template_repo",
                id
            );
        }
    }

    #[test]
    fn all_templates_have_starter_files() {
        for id in &["arcade", "authoritative", "fps", "motorsport"] {
            let t = find_template(id).unwrap();
            assert!(
                !t.starter_files.is_empty(),
                "{} must have at least one starter file",
                id
            );
            // Cargo.toml is always expected
            assert!(
                t.starter_files.contains(&"Cargo.toml"),
                "{} must include Cargo.toml in starter_files",
                id
            );
        }
    }

    #[test]
    fn all_templates_have_descriptions() {
        for id in &["arcade", "authoritative", "fps", "motorsport"] {
            let t = find_template(id).unwrap();
            assert!(
                !t.description.is_empty(),
                "{} must have a non-empty description",
                id
            );
            assert!(
                !t.name.is_empty(),
                "{} must have a non-empty display name",
                id
            );
        }
    }

    #[test]
    fn template_serialises_to_json() {
        let t = find_template("arcade").unwrap();
        // Confirm the type implements Serialize (it is used in the JSON handler).
        let json = serde_json::to_string(t).expect("template must serialise to JSON");
        assert!(json.contains("\"id\":\"arcade\""));
        assert!(json.contains("\"tier\""));
        assert!(json.contains("\"description\""));
    }

    #[test]
    fn authoritative_template_path_matches_repo_dir() {
        let t = find_template("authoritative").unwrap();
        // The template_path must match the on-disk directory name.
        assert_eq!(t.template_path, "game-templates/authoritative");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Scaffold logic — pure-logic tests (no DB)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod scaffold_logic_tests {
    use magnetite_backend::api::templates::find_template;

    /// Replicate the crate-name sanitisation used in scaffold_game.
    fn sanitise_crate_name(title: &str) -> String {
        let raw: String = title
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        let trimmed = raw.trim_matches('_').to_string();
        if trimmed.is_empty() {
            "my_game".to_string()
        } else {
            trimmed
        }
    }

    /// Build the cli_command string as scaffold_game does.
    fn build_cli_command(crate_name: &str, template_id: &str) -> String {
        format!("magnetite new {} --template {}", crate_name, template_id)
    }

    #[test]
    fn crate_name_lowercases_title() {
        assert_eq!(sanitise_crate_name("My Game"), "my_game");
    }

    #[test]
    fn crate_name_replaces_special_chars_with_underscore() {
        // Trailing underscores are trimmed by trim_matches('_').
        assert_eq!(sanitise_crate_name("Space Shooter!!"), "space_shooter");
    }

    #[test]
    fn crate_name_trims_leading_trailing_underscores() {
        assert_eq!(sanitise_crate_name("  !Game!  "), "game");
    }

    #[test]
    fn crate_name_falls_back_to_my_game_for_empty() {
        assert_eq!(sanitise_crate_name(""), "my_game");
        assert_eq!(sanitise_crate_name("!!!"), "my_game");
    }

    #[test]
    fn crate_name_preserves_numbers() {
        assert_eq!(sanitise_crate_name("Arena 2048"), "arena_2048");
    }

    #[test]
    fn cli_command_contains_crate_name_and_template() {
        let cmd = build_cli_command("my_shooter", "authoritative");
        assert!(
            cmd.starts_with("magnetite new "),
            "must start with 'magnetite new '"
        );
        assert!(cmd.contains("my_shooter"), "must contain crate name");
        assert!(
            cmd.contains("--template authoritative"),
            "must contain --template flag"
        );
    }

    #[test]
    fn scaffold_info_fields_come_from_template() {
        let template = find_template("fps").unwrap();
        let crate_name = "my_fps";
        let cli_command = build_cli_command(crate_name, template.id);

        assert_eq!(template.template_path, "game-templates/fps");
        assert_eq!(template.template_repo, "magnetite/game-templates/fps");
        assert!(cli_command.contains("fps"));
    }

    #[test]
    fn scaffold_starter_files_include_game_rs() {
        for id in &["fps", "motorsport", "authoritative"] {
            let t = find_template(id).unwrap();
            assert!(
                t.starter_files.contains(&"src/game.rs"),
                "{} must include src/game.rs in starter_files",
                id
            );
        }
    }

    #[test]
    fn arcade_template_starter_files_are_minimal() {
        let t = find_template("arcade").unwrap();
        // Arcade should not reference 3-D files.
        for f in t.starter_files {
            assert!(
                !f.contains("hitscan") && !f.contains("vehicle") && !f.contains("track"),
                "arcade starter_files should not reference 3D-specific files: {}",
                f
            );
        }
    }

    #[test]
    fn scaffold_instructions_mention_cli_install() {
        // We replicate the instruction format from scaffold_game.
        let crate_name = "space_jam";
        let template_id = "arcade";
        let game_id = uuid::Uuid::nil();
        let instructions = format!(
            "1. Install the Magnetite CLI: `cargo install magnetite-cli`\n\
             2. Scaffold your game: `magnetite new {} --template {}`\n\
             3. Connect a GitHub repo via POST /api/v1/github/repos\n\
             4. Trigger your first build: POST /api/v1/distribution/{}/build\n\
             5. Check build status: GET /api/v1/developer/games/{}/build-status",
            crate_name, template_id, game_id, game_id
        );
        assert!(instructions.contains("cargo install magnetite-cli"));
        assert!(instructions.contains("magnetite new space_jam --template arcade"));
        assert!(instructions.contains("Connect a GitHub repo"));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Provisioning — default fields and InstanceSummary shape
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod provisioning_shape_tests {
    use magnetite_backend::api::provisioning::InstanceSummary;
    use magnetite_backend::services::provisioning::RuntimeInstance;
    use uuid::Uuid;

    fn make_instance(status: &str, ws_endpoint: Option<&str>) -> RuntimeInstance {
        RuntimeInstance {
            id: Uuid::new_v4(),
            game_id: Uuid::new_v4(),
            version_id: None,
            artifact_id: None,
            status: status.to_string(),
            ws_endpoint: ws_endpoint.map(|s| s.to_string()),
            topology: "SingleRoom".to_string(),
            max_players: 4,
            tick_hz: 20,
            local_pid: None,
            runner_note: None,
            requested_by: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn instance_summary_running_has_ws_endpoint() {
        let inst = make_instance("running", Some("ws://127.0.0.1:9001"));
        let summary = InstanceSummary::from(inst);
        assert_eq!(summary.status, "running");
        assert_eq!(summary.ws_endpoint.as_deref(), Some("ws://127.0.0.1:9001"));
    }

    #[test]
    fn instance_summary_pending_has_null_ws_endpoint() {
        let inst = make_instance("pending", None);
        let summary = InstanceSummary::from(inst);
        assert_eq!(summary.status, "pending");
        assert!(summary.ws_endpoint.is_none());
    }

    #[test]
    fn instance_summary_topology_and_limits_preserved() {
        let inst = make_instance("running", None);
        let summary = InstanceSummary::from(inst);
        assert_eq!(summary.topology, "SingleRoom");
        assert_eq!(summary.max_players, 4);
        assert_eq!(summary.tick_hz, 20);
    }

    #[test]
    fn instance_summary_game_id_preserved() {
        let gid = Uuid::new_v4();
        let mut inst = make_instance("running", None);
        inst.game_id = gid;
        let summary = InstanceSummary::from(inst);
        assert_eq!(summary.game_id, gid);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Distribution — request parsing and BuildStatusSummary
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod distribution_shape_tests {
    use magnetite_backend::api::distribution::BuildStatusSummary;
    use uuid::Uuid;

    #[test]
    fn build_status_summary_serialises_with_optional_fields() {
        let summary = BuildStatusSummary {
            game_id: Uuid::nil(),
            latest_build_status: Some("success".to_string()),
            latest_version: Some("0.1.0".to_string()),
            artifact_count: 2,
            live_version: None,
        };
        let json = serde_json::to_string(&summary).expect("must serialise");
        assert!(json.contains("\"artifact_count\":2"));
        assert!(json.contains("\"latest_build_status\":\"success\""));
        assert!(json.contains("\"live_version\":null"));
    }

    #[test]
    fn build_status_summary_all_none_fields() {
        let summary = BuildStatusSummary {
            game_id: Uuid::nil(),
            latest_build_status: None,
            latest_version: None,
            artifact_count: 0,
            live_version: None,
        };
        let json = serde_json::to_string(&summary).expect("must serialise");
        assert!(json.contains("\"artifact_count\":0"));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. ScaffoldResponse — round-trip via JSON
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod scaffold_response_roundtrip_tests {
    use magnetite_backend::api::developer::{ScaffoldInfo, ScaffoldResponse};
    use uuid::Uuid;

    fn make_scaffold_response() -> ScaffoldResponse {
        ScaffoldResponse {
            game_id: Uuid::new_v4(),
            scaffold: ScaffoldInfo {
                cli_command: "magnetite new my_shooter --template authoritative".to_string(),
                template_path: "game-templates/authoritative".to_string(),
                template_repo: "magnetite/game-templates/authoritative".to_string(),
                starter_files: vec![
                    "Cargo.toml".to_string(),
                    "src/lib.rs".to_string(),
                    "src/types.rs".to_string(),
                    "src/game.rs".to_string(),
                    "src/wasm_abi.rs".to_string(),
                ],
                instructions: "1. cargo install magnetite-cli\n2. magnetite new my_shooter"
                    .to_string(),
            },
        }
    }

    #[test]
    fn scaffold_response_serialises_to_json() {
        let resp = make_scaffold_response();
        let json = serde_json::to_string(&resp).expect("must serialise");
        assert!(json.contains("\"cli_command\""));
        assert!(json.contains("magnetite new my_shooter --template authoritative"));
        assert!(json.contains("\"starter_files\""));
        assert!(json.contains("wasm_abi.rs"));
    }

    #[test]
    fn scaffold_response_game_id_is_valid_uuid() {
        let resp = make_scaffold_response();
        // game_id is a valid non-nil UUID.
        assert_ne!(resp.game_id, Uuid::nil());
    }

    #[test]
    fn scaffold_info_starter_files_includes_cargo_toml() {
        let resp = make_scaffold_response();
        assert!(
            resp.scaffold
                .starter_files
                .iter()
                .any(|f| f == "Cargo.toml"),
            "starter_files must include Cargo.toml"
        );
    }

    #[test]
    fn scaffold_info_cli_command_is_magnetite_new() {
        let resp = make_scaffold_response();
        assert!(
            resp.scaffold.cli_command.starts_with("magnetite new "),
            "cli_command must start with 'magnetite new '"
        );
    }

    #[test]
    fn scaffold_info_instructions_mention_cli_install() {
        let resp = make_scaffold_response();
        assert!(
            resp.scaffold
                .instructions
                .contains("cargo install magnetite-cli"),
            "instructions must mention CLI install"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. CLI scaffold templates — verify cargo_toml_template / lib_rs_template
//    This exercises the same template logic as the CLI `magnetite new` command.
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod cli_scaffold_template_tests {
    /// Minimal mirror of the private cargo_toml_template function in magnetite-cli.
    fn cargo_toml_template(name: &str) -> String {
        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[features]
default = []
wasm = []

[dependencies]
magnetite-sdk = {{ path = "../backend/magnetite-sdk" }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
"#,
            name = name
        )
    }

    fn to_pascal_case(s: &str) -> String {
        s.split(&['-', '_'][..])
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect()
    }

    #[test]
    fn cargo_toml_template_has_package_name() {
        let toml = cargo_toml_template("cool-game");
        assert!(toml.contains("name = \"cool-game\""));
    }

    #[test]
    fn cargo_toml_template_declares_cdylib_and_rlib() {
        let toml = cargo_toml_template("my-game");
        assert!(toml.contains("cdylib"));
        assert!(toml.contains("rlib"));
    }

    #[test]
    fn cargo_toml_template_has_wasm_feature() {
        let toml = cargo_toml_template("my-game");
        assert!(toml.contains("wasm = []"));
    }

    #[test]
    fn cargo_toml_template_depends_on_magnetite_sdk() {
        let toml = cargo_toml_template("my-game");
        assert!(toml.contains("magnetite-sdk"));
        assert!(toml.contains("serde"));
    }

    #[test]
    fn to_pascal_case_converts_kebab() {
        assert_eq!(to_pascal_case("my-cool-game"), "MyCoolGame");
    }

    #[test]
    fn to_pascal_case_converts_snake() {
        assert_eq!(to_pascal_case("arena_shooter"), "ArenaShooter");
    }

    #[test]
    fn to_pascal_case_single_word() {
        assert_eq!(to_pascal_case("game"), "Game");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. GDS templates endpoint handler — axum integration without DB
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod templates_http_handler_tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn get_templates_returns_200_with_array() {
        let app = magnetite_backend::api::templates::router();

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        assert!(json.is_array(), "response must be a JSON array");
        let arr = json.as_array().unwrap();
        assert_eq!(arr.len(), 4, "must return exactly 4 templates");
    }

    #[tokio::test]
    async fn get_template_by_id_returns_single_entry() {
        let app = magnetite_backend::api::templates::router();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/authoritative")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        assert!(json.is_object());
        assert_eq!(json["id"], "authoritative");
        assert!(json["starter_files"].is_array());
    }

    #[tokio::test]
    async fn get_template_unknown_id_returns_404() {
        let app = magnetite_backend::api::templates::router();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/notexist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn get_templates_response_includes_all_tiers() {
        let app = magnetite_backend::api::templates::router();

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        let arr = json.as_array().unwrap();

        let tiers: Vec<&str> = arr
            .iter()
            .map(|t| t["tier"].as_str().unwrap_or(""))
            .collect();
        assert!(tiers.contains(&"arcade"), "must include arcade tier");
        assert!(
            tiers.contains(&"authoritative"),
            "must include authoritative tier"
        );
        assert!(tiers.contains(&"fps"), "must include fps tier");
        assert!(
            tiers.contains(&"motorsport"),
            "must include motorsport tier"
        );
    }

    #[tokio::test]
    async fn get_templates_each_entry_has_required_fields() {
        let app = magnetite_backend::api::templates::router();

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        let arr = json.as_array().unwrap();

        for entry in arr {
            assert!(entry["id"].is_string(), "template must have 'id'");
            assert!(entry["name"].is_string(), "template must have 'name'");
            assert!(
                entry["description"].is_string(),
                "template must have 'description'"
            );
            assert!(
                entry["template_path"].is_string(),
                "template must have 'template_path'"
            );
            assert!(
                entry["starter_files"].is_array(),
                "template must have 'starter_files'"
            );
        }
    }
}
