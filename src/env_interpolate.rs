//! Environment variable interpolation for MCP configuration values.
//!
//! Supports `${VAR}` syntax in command strings, args, and env values.
//! When `expand` is true, `${VAR}` references are replaced with the
//! corresponding environment variable value. Unset variables are kept
//! as-is.

use std::collections::BTreeMap;

use crate::config::{McpConfig, McpServer};

/// Interpolate `${VAR}` patterns in a string.
///
/// When `expand` is false, the string is returned unchanged.
/// When `expand` is true, `${VAR}` references are replaced with the
/// value of the environment variable `VAR`. If `VAR` is not set, the
/// original `${VAR}` text is preserved.
pub fn interpolate_env(value: &str, expand: bool) -> String {
    if !expand {
        return value.to_string();
    }
    expand_env_vars(value)
}

/// Replace all `${VAR}` occurrences with the corresponding env value.
fn expand_env_vars(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1] == '{' {
            let start = i;
            i += 2; // skip ${
            let mut var_name = String::new();
            let mut found_close = false;
            while i < chars.len() {
                if chars[i] == '}' {
                    found_close = true;
                    i += 1;
                    break;
                }
                var_name.push(chars[i]);
                i += 1;
            }
            if found_close {
                match std::env::var(&var_name) {
                    Ok(val) => result.push_str(&val),
                    Err(_) => result.push_str(&format!("${{{var_name}}}")),
                }
            } else {
                // Unclosed ${ — preserve original text
                for &ch in chars.iter().take(i).skip(start) {
                    result.push(ch);
                }
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

/// Interpolate environment variables in all fields of an [`McpConfig`].
///
/// Returns a new [`McpConfig`] with every string field processed through
/// [`interpolate_env`].
pub fn interpolate_mcp_config(config: &McpConfig, expand: bool) -> McpConfig {
    let mut servers = BTreeMap::new();
    for (name, server) in &config.servers {
        servers.insert(
            name.clone(),
            McpServer {
                command: interpolate_env(&server.command, expand),
                args: server
                    .args
                    .iter()
                    .map(|a| interpolate_env(a, expand))
                    .collect(),
                env: server
                    .env
                    .iter()
                    .map(|(k, v)| (k.clone(), interpolate_env(v, expand)))
                    .collect(),
            },
        );
    }
    McpConfig { servers }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_utils::make_server;

    #[test]
    fn interpolate_no_expand_returns_same() {
        let input = "hello ${WORLD}";
        let result = interpolate_env(input, false);
        assert_eq!(result, input);
    }

    #[test]
    fn interpolate_expands_set_var() {
        unsafe { std::env::set_var("LORUM_TEST_VAR", "expanded") };
        let result = interpolate_env("prefix_${LORUM_TEST_VAR}_suffix", true);
        assert_eq!(result, "prefix_expanded_suffix");
        unsafe { std::env::remove_var("LORUM_TEST_VAR") };
    }

    #[test]
    fn interpolate_keeps_unset_var() {
        unsafe { std::env::remove_var("LORUM_NONEXISTENT_VAR") };
        let result = interpolate_env("${LORUM_NONEXISTENT_VAR}", true);
        assert_eq!(result, "${LORUM_NONEXISTENT_VAR}");
    }

    #[test]
    fn interpolate_no_vars() {
        let result = interpolate_env("plain text", true);
        assert_eq!(result, "plain text");
    }

    #[test]
    fn interpolate_empty_string() {
        let result = interpolate_env("", true);
        assert_eq!(result, "");
    }

    #[test]
    fn interpolate_multiple_vars() {
        unsafe { std::env::set_var("LORUM_A", "1") };
        unsafe { std::env::set_var("LORUM_B", "2") };
        let result = interpolate_env("${LORUM_A}_${LORUM_B}", true);
        assert_eq!(result, "1_2");
        unsafe { std::env::remove_var("LORUM_A") };
        unsafe { std::env::remove_var("LORUM_B") };
    }

    #[test]
    fn interpolate_unclosed_brace() {
        let result = interpolate_env("${UNCLOSED", true);
        // Without a closing '}', the original text is preserved as-is.
        assert_eq!(result, "${UNCLOSED");
    }

    #[test]
    fn interpolate_dollar_without_brace() {
        let result = interpolate_env("$HOME/path", true);
        // '$' followed by non-'{' is treated as literal
        assert_eq!(result, "$HOME/path");
    }

    #[test]
    fn interpolate_mcp_config_expands_all_fields() {
        unsafe { std::env::set_var("LORUM_CMD", "run") };
        unsafe { std::env::set_var("LORUM_ARG", "flag") };
        unsafe { std::env::set_var("LORUM_ENV_VAL", "secret") };

        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert(
                    "test".into(),
                    McpServer {
                        command: "${LORUM_CMD}".into(),
                        args: vec!["${LORUM_ARG}".into()],
                        env: {
                            let mut e = BTreeMap::new();
                            e.insert("KEY".into(), "${LORUM_ENV_VAL}".into());
                            e
                        },
                    },
                );
                m
            },
        };

        let result = interpolate_mcp_config(&config, true);
        let server = &result.servers["test"];
        assert_eq!(server.command, "run");
        assert_eq!(server.args, vec!["flag"]);
        assert_eq!(server.env.get("KEY").unwrap(), "secret");

        unsafe { std::env::remove_var("LORUM_CMD") };
        unsafe { std::env::remove_var("LORUM_ARG") };
        unsafe { std::env::remove_var("LORUM_ENV_VAL") };
    }

    #[test]
    fn interpolate_mcp_config_no_expand_preserves() {
        let server = make_server("${VAR}", &["${ARG}"], &[("K", "${VAL}")]);
        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("s".into(), server);
                m
            },
        };
        let result = interpolate_mcp_config(&config, false);
        assert_eq!(result, config);
    }
}
