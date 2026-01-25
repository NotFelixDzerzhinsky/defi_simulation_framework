use std::fs;

use dex_sim::config::load_dex_config;
use tempfile::tempdir;

#[test]
fn parses_valid_dex_config() {
    let tempdir = tempdir().expect("tempdir should be created");
    let path = tempdir.path().join("dex.toml");

    fs::write(
        &path,
        r#"
name = "mini-v2"
adapter = "builtin.v2"

[contracts]
router = "0x1111111111111111111111111111111111111111"
factory = "0x2222222222222222222222222222222222222222"

[assets]
abi_dir = "abi"
artifacts_dir = "artifacts"

[[tokens]]
symbol = "USDC"
address = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
decimals = 6

[[tokens]]
symbol = "WETH"
address = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
decimals = 18

[protocol]
fee_bps = 30
"#,
    )
    .expect("config should be written");

    let config = load_dex_config(&path).expect("config should parse");

    assert_eq!(config.name, "mini-v2");
    assert_eq!(config.adapter, "builtin.v2");
    assert_eq!(config.tokens.len(), 2);
    assert_eq!(
        config
            .protocol
            .get("fee_bps")
            .and_then(|value| value.as_integer()),
        Some(30)
    );
}

#[test]
fn rejects_duplicate_token_symbols() {
    let tempdir = tempdir().expect("tempdir should be created");
    let path = tempdir.path().join("dex.toml");

    fs::write(
        &path,
        r#"
name = "broken-dex"
adapter = "builtin.v2"

[contracts]
router = "0x1111111111111111111111111111111111111111"

[[tokens]]
symbol = "USDC"
address = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
decimals = 6

[[tokens]]
symbol = "usdc"
address = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
decimals = 18
"#,
    )
    .expect("config should be written");

    let error = load_dex_config(&path).expect_err("config should fail validation");

    assert!(error.to_string().contains("duplicate token symbol"));
}
