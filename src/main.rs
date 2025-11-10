//
// Copyright (c) 2025 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
//
// Contributors:
//   Julien Enoch, <julien.enoch@zettascale.tech>
//
use std::{path::PathBuf, str::FromStr};

use anyhow::anyhow;
use futures::select;
use strum::{EnumString, VariantNames};
use zenoh::{
    self,
    bytes::Encoding,
    internal::{plugins::PluginsManager, runtime::RuntimeBuilder},
    key_expr::format::{kedefine, keformat},
    query::Query,
};

mod args;
mod field_type;
mod registry;
mod type_description;
mod type_info;

// Key expression for the Liveliness Token assessing this types registry is up and running
const KE_LIVELINESS_TOKEN: &str = "@ros2_types";

kedefine!(
    // Key expression pattern for the Queryable on types
    pub(crate) keformat_ros2_types: "@ros2_types/${type_name:**}",
    // Key expression pattern for the Queryable on environment variables
    pub(crate) keformat_ros2_env: "@ros2_env/${env_var:*}",
);

// List of environment variables that can be queried via the @ros2_env/* queryable
// If the queried variable is not in this list, an error is returned.
const ALLOWED_ENV_VARS: &[&str] = &[
    "ROS_DOMAIN_ID",
    "RMW_IMPLEMENTATION",
    "ROS_VERSION",
    "ROS_PYTHON_VERSION",
    "ROS_DISTRO",
    "AMENT_PREFIX_PATH",
];

#[derive(Debug, Default, Clone, Copy, EnumString, PartialEq, Eq, VariantNames)]
#[strum(ascii_case_insensitive)]
pub(crate) enum ReplyFormat {
    #[default]
    TypeDescription, // the type description in JSON
    FullTypeDescription, // the full type description with dependencies in JSON
    Definition,          // the original .msg/.srv/.action definition
    Mcap,                // the type description for a MCAP schema
    Hash,                // the type hash string
    Path,                // the path to the original .msg/.srv/.action file
}

fn get_ament_share_paths() -> Vec<PathBuf> {
    match std::env::var("AMENT_PREFIX_PATH") {
        Err(_) => {
            tracing::error!("AMENT_PREFIX_PATH environment variable is not defined. Is your ROS environment setup ?");
            std::process::exit(-1);
        }
        Ok(s) if s.is_empty() => {
            tracing::error!("AMENT_PREFIX_PATH environment variable is empty. Is your ROS environment correctly setup ?");
            std::process::exit(-1);
        }
        Ok(ament_prefix_path) => ament_prefix_path
            .split(':')
            .map(|p| {
                let mut path = PathBuf::from(p);
                path.push("share");
                path
            })
            .collect(),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // initiate logging
    zenoh::init_log_from_env_or("info");

    // parse command line arguments
    let config = args::parse_args();

    // Plugin manager with REST plugin
    let mut plugins_manager = PluginsManager::static_plugins_only();
    if let Ok(http_port) = config.get_json("plugins/rest/http_port") {
        tracing::info!("REST plugin available on HTTP port {http_port}");
        plugins_manager.declare_static_plugin::<zenoh_plugin_rest::RestPlugin, &str>("rest", true);
    }

    // Create a Zenoh Runtime with the PluginManager and a Session.
    let mut runtime = RuntimeBuilder::new(config)
        .plugins_manager(plugins_manager)
        .build()
        .await
        .map_err(|err| anyhow!("failed to build Zenoh runtime: {err}"))?;
    runtime
        .start()
        .await
        .map_err(|err| anyhow!("failed to start Zenoh runtime: {err}"))?;
    let session = zenoh::session::init(runtime.into())
        .await
        .map_err(|err| anyhow!("failed to create Zenoh session: {err}"))?;

    // Create Registry and load all types
    let mut registry = registry::Registry::new();
    for path in get_ament_share_paths() {
        registry.load_types_from_dir(&path);
    }
    tracing::info!("Total types in registry: {}", registry.get_size());

    // Declare Queryable for types
    let ros2_types_queryable_ke = keformat!(keformat_ros2_types::formatter(), type_name = "**")
        .map_err(|err| {
            anyhow!(
                "Internal error that shouldn't happen, formating ros2_types_queryable_ke: {err}"
            )
        })?;
    tracing::debug!("Declaring Queryable on '{ros2_types_queryable_ke}'");
    let ros2_types_queryable = session
        .declare_queryable(ros2_types_queryable_ke)
        .await
        .map_err(|err| anyhow!("failed to declare queryable for types: {err}"))?;

    // Declare Queryable for environment variables
    let ros2_env_queryable_ke =
        keformat!(keformat_ros2_env::formatter(), env_var = "*").map_err(|err| {
            anyhow!("Internal error that shouldn't happen, formating ros2_env_queryable_ke: {err}")
        })?;
    tracing::debug!("Declaring Queryable on '{ros2_env_queryable_ke}'");
    let ros2_env_queryable = session
        .declare_queryable(ros2_env_queryable_ke)
        .await
        .map_err(|err| anyhow!("failed to declare queryable for environment variables: {err}"))?;

    // Declare the Liveliness Token
    let _liveliness_token = session
        .liveliness()
        .declare_token(KE_LIVELINESS_TOKEN)
        .await
        .map_err(|err| anyhow!("failed to create Liveliness Token: {err}"))?;

    tracing::info!("Ready! Listening for queries...");
    loop {
        // Wait a query
        select!(
            query = ros2_types_queryable.recv_async() => {
                if let Ok(q) = query {
                    handle_ros2_types_query(q, &registry).await;
                } else {
                    tracing::error!("Query recceived but ros2_types_queryable was closed");
                }
            },
            query = ros2_env_queryable.recv_async() => {
                if let Ok(q) = query {
                    handle_ros2_env_query(q).await;
                } else {
                    tracing::error!("Query recceived but ros2_env_queryable was closed");
                }
            },
        )
    }
}

async fn handle_ros2_types_query(query: Query, registry: &registry::Registry<'_>) {
    tracing::debug!("Received query: {}", query.key_expr());
    let ke = match keformat_ros2_types::parse(query.key_expr()) {
        Ok(ke) => ke,
        Err(_) => {
            tracing::error!(
                "Received a query on '{}' but it doesn't match the '@ros2_types/**' queryable!",
                query.key_expr()
            );
            return;
        }
    };

    let format = match query.parameters().get("format") {
        Some(f) => match ReplyFormat::from_str(f) {
            Ok(fmt) => fmt,
            Err(_) => {
                query
                    .reply_err(format!(
                        "Unknown format '{f}' - accepted values are: {:?}",
                        ReplyFormat::VARIANTS
                    ))
                    .await
                    .unwrap_or_else(|e| {
                        tracing::warn!("Error sending reply for {}: {e}", query.key_expr())
                    });
                return;
            }
        },
        None => ReplyFormat::default(),
    };

    if let Some(type_name) = ke.type_name() {
        let types = registry.get_types(type_name);
        tracing::debug!("Found {} types matching {}", types.len(), type_name);

        for type_info in types {
            let reply_ke = keformat!(
                keformat_ros2_types::formatter(),
                type_name = &type_info.full_name
            )
            .expect("Shouldn't happen: all parameters are valid keyexpr!");
            match format {
                ReplyFormat::TypeDescription => {
                    let response = serde_json::to_string(
                        &type_info
                            .type_description
                            .type_description_msg
                            .type_description,
                    )
                    .unwrap_or_else(|e| format!("Failed to serialize type description: {e}"));
                    query
                        .reply(reply_ke, response)
                        .encoding(Encoding::APPLICATION_JSON)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::warn!("Error sending reply for {}: {e}", query.key_expr())
                        });
                }

                ReplyFormat::FullTypeDescription => {
                    let response =
                        serde_json::to_string(&type_info.type_description.type_description_msg)
                            .unwrap_or_else(|e| {
                                format!("Failed to serialize type description: {e}")
                            });
                    query
                        .reply(reply_ke, response)
                        .encoding(Encoding::APPLICATION_JSON)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::warn!("Error sending reply for {}: {e}", query.key_expr())
                        });
                }

                ReplyFormat::Definition => {
                    query
                        .reply(reply_ke, &type_info.definition_content)
                        .encoding(Encoding::TEXT_PLAIN)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::warn!("Error sending reply for {}: {e}", query.key_expr())
                        });
                }

                ReplyFormat::Mcap => {
                    query
                        .reply(reply_ke, registry.get_mcap_schema(type_info))
                        .encoding(Encoding::TEXT_PLAIN)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::warn!("Error sending reply for {}: {e}", query.key_expr())
                        });
                }

                ReplyFormat::Hash => {
                    query
                        .reply(reply_ke, &type_info.type_hash)
                        .encoding(Encoding::TEXT_PLAIN)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::warn!("Error sending reply for {}: {e}", query.key_expr())
                        });
                }

                ReplyFormat::Path => {
                    query
                        .reply(reply_ke, type_info.definition_path.to_string_lossy())
                        .encoding(Encoding::TEXT_PLAIN)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::warn!("Error sending reply for {}: {e}", query.key_expr())
                        });
                }
            }
        }
    }
}

async fn handle_ros2_env_query(query: Query) {
    tracing::debug!("Received query: {}", query.key_expr());
    let ke = match keformat_ros2_env::parse(query.key_expr()) {
        Ok(ke) => ke,
        Err(_) => {
            tracing::error!(
                "Received a query on '{}' but it doesn't match the '@ros2_env/*' queryable!",
                query.key_expr()
            );
            return;
        }
    };

    if ALLOWED_ENV_VARS.contains(&ke.env_var().as_str()) {
        if let Some(value) = std::env::var_os(ke.env_var().as_str()) {
            query
                .reply(query.key_expr(), value.to_string_lossy())
                .encoding(Encoding::TEXT_PLAIN)
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!("Error sending reply for {}: {e}", query.key_expr())
                });
        }
    } else {
        query
            .reply_err(format!(
                "Environment variable '{}' cannot be queried. Allowed variables are: {:?}",
                ke.env_var(),
                ALLOWED_ENV_VARS
            ))
            .encoding(Encoding::TEXT_PLAIN)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("Error sending reply for {}: {e}", query.key_expr())
            });
    }
}
