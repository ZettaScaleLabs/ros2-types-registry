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

use anyhow::anyhow;
use std::{path::PathBuf, str::FromStr};
use strum::{EnumString, VariantNames};
use zenoh::{
    self,
    bytes::Encoding,
    internal::{plugins::PluginsManager, runtime::RuntimeBuilder},
    key_expr::format::{kedefine, keformat},
};

mod args;
mod field_type;
mod registry;
mod type_description;
mod type_info;

kedefine!(
    pub(crate) ke_queries: "@ros2_types/${type_name:**}",
);

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

    let mut registry = registry::Registry::new();
    for path in get_ament_share_paths() {
        registry.load_types_from_dir(&path);
    }
    tracing::info!("Total types in registry: {}", registry.get_size());

    // parse command line arguments
    let config = args::parse_args();

    // Plugin manager with REST plugin
    let mut plugins_manager = PluginsManager::static_plugins_only();
    plugins_manager.declare_static_plugin::<zenoh_plugin_rest::RestPlugin, &str>("rest", true);

    // Create a Zenoh Runtime with the PluginManager and a Session.
    tracing::debug!("Opening session...");
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



    let queryable_key_expr = keformat!(ke_queries::formatter(), type_name = "**").unwrap();

    tracing::debug!("Declaring Queryable on '{queryable_key_expr}'...");
    let queryable = session.declare_queryable(queryable_key_expr).await.unwrap();

    tracing::info!("Ready! Listening for queries...");

    while let Ok(query) = queryable.recv_async().await {
        tracing::debug!("Received query: {}", query.key_expr());
        let ke = ke_queries::parse(query.key_expr()).unwrap();

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
                    continue;
                }
            },
            None => ReplyFormat::default(),
        };

        if let Some(type_name) = ke.type_name() {
            let types = registry.get_types(type_name);
            tracing::debug!("Found {} types matching {}", types.len(), type_name);

            for type_info in types {
                let reply_ke = keformat!(ke_queries::formatter(), type_name = &type_info.full_name)
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

    Ok(())
}
