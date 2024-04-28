use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// `GET`
pub mod get {
    use super::*;

    /// `GET /api`
    pub mod api {
        use super::*;

        /// `GET /api/remote`
        pub mod remote {
            use super::*;

            /// `GET /api/remote/servers`
            pub mod servers {
                use super::*;

                /// Query parameters for `GET /api/remote/servers`.
                #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                pub struct QueryParams {
                    pub page: Option<u32>,
                    pub per_page: Option<u32>,
                }

                /// `GET /api/remote/servers`
                #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                pub struct Response {
                    pub data: Vec<response::Data>,
                    pub links: response::Links,
                    pub meta: response::Meta,
                }

                /// Data for the response to `GET /api/remote/servers`.
                pub mod response {
                    use super::*;

                    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                    pub struct Data {
                        pub uuid: Uuid,
                        pub settings: data::Settings,
                        pub process_configuration: data::ProcessConfiguration,
                    }

                    pub mod data {
                        use super::*;

                        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                        pub struct Settings {
                            pub uuid: String,
                            pub meta: settings::Meta,
                            pub suspended: bool,
                            pub environment: settings::Environment,
                            pub invocation: String,
                            pub skip_egg_scripts: bool,
                            pub build: settings::Build,
                            pub container: settings::Container,
                            pub allocations: settings::Allocations,
                            pub mounts: Vec<String>,
                            pub egg: settings::Egg,
                        }

                        pub mod settings {
                            use super::*;

                            #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                            pub struct Meta {
                                pub name: String,
                                pub description: String,
                            }

                            #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                            pub struct Environment {
                                #[serde(rename = "SERVER_JARFILE")]
                                pub server_jarfile: String,
                                #[serde(rename = "MC_VERSION")]
                                pub mc_version: String,
                                #[serde(rename = "BUILD_TYPE")]
                                pub build_type: String,
                                #[serde(rename = "FORGE_VERSION")]
                                pub forge_version: String,
                                #[serde(rename = "STARTUP")]
                                pub startup: String,
                                #[serde(rename = "P_SERVER_LOCATION")]
                                pub p_server_location: String,
                                #[serde(rename = "P_SERVER_UUID")]
                                pub p_server_uuid: String,
                                #[serde(rename = "P_SERVER_ALLOCATION_LIMIT")]
                                pub p_server_allocation_limit: i64,
                            }

                            #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                            pub struct Build {
                                pub memory_limit: i64,
                                pub swap: i64,
                                pub io_weight: i64,
                                pub cpu_limit: i64,
                                pub threads: u16,
                                pub disk_space: i64,
                                pub oom_disabled: bool,
                            }

                            #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                            pub struct Container {
                                pub image: String,
                                pub oom_disabled: bool,
                                pub requires_rebuild: bool,
                            }

                            #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                            pub struct Allocations {
                                pub force_outgoing_ip: bool,
                                pub default: allocations::Default,
                                pub mappings: serde_json::Map<String, serde_json::Value>,
                            }

                            pub mod allocations {
                                use super::*;

                                #[derive(
                                    Default, Debug, Clone, PartialEq, Serialize, Deserialize,
                                )]
                                pub struct Default {
                                    pub ip: String,
                                    pub port: i64,
                                }
                            }

                            #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                            pub struct Egg {
                                pub id: String,
                                pub file_denylist: Vec<String>,
                            }
                        }

                        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                        pub struct ProcessConfiguration {
                            pub startup: process_configuration::Startup,
                            pub stop: process_configuration::Stop,
                            pub configs: Vec<process_configuration::Config>,
                        }

                        pub mod process_configuration {
                            use super::*;

                            #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                            pub struct Startup {
                                pub done: Vec<String>,
                                pub user_interaction: Vec<String>,
                                pub strip_ansi: bool,
                            }

                            #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                            pub struct Stop {
                                #[serde(rename = "type")]
                                pub type_field: String,
                                pub value: String,
                            }

                            #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                            pub struct Config {
                                pub parser: String,
                                pub file: String,
                                pub replace: Vec<config::Replace>,
                            }

                            pub mod config {
                                use super::*;

                                #[derive(
                                    Default, Debug, Clone, PartialEq, Serialize, Deserialize,
                                )]
                                pub struct Replace {
                                    #[serde(rename = "match")]
                                    pub match_field: String,
                                    pub replace_with: String,
                                }
                            }
                        }
                    }

                    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                    pub struct Links {
                        pub first: String,
                        pub last: String,
                        pub prev: Option<String>,
                        pub next: Option<String>,
                    }

                    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                    pub struct Meta {
                        pub current_page: i64,
                        pub from: i64,
                        pub last_page: i64,
                        pub links: Vec<meta::Link>,
                        pub path: String,
                        pub per_page: i64,
                        pub to: i64,
                        pub total: i64,
                    }

                    pub mod meta {
                        use super::*;

                        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                        pub struct Link {
                            pub url: Option<String>,
                            pub label: String,
                            pub active: bool,
                        }
                    }
                }

                /// `GET /api/client/servers/:uuid`
                pub mod uuid {
                    use super::*;

                    /// Path parameters for `GET /api/client/servers/:uuid`
                    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                    pub struct Path {
                        pub uuid: Uuid,
                    }

                    /// Response for `GET /api/client/servers/:uuid`
                    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                    pub struct Response {
                        settings: super::response::data::Settings,
                        process_configuration: super::response::data::ProcessConfiguration,
                    }

                    /// `GET /api/client/servers/:uuid/install`
                    pub mod install {
                        use super::*;

                        /// Path parameters for `GET /api/client/servers/:uuid/install`
                        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                        pub struct Response {
                            pub container_image: String,
                            pub entrypoint: String,
                            pub script: String,
                        }
                    }
                }
            }
        }
    }
}

/// `POST`
pub mod post {
    use super::*;

    /// `POST /api`
    pub mod api {
        use super::*;

        /// `POST /api/remote`
        pub mod remote {
            use super::*;

            /// `POST /api/remote/servers`
            pub mod servers {
                use super::*;

                /// `POST /api/remote/servers/reset`
                pub mod reset {
                    use super::*;

                    /// Response for `POST /api/remote/servers/reset`
                    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                    pub struct Response;
                }

                /// `POST /api/remote/servers/:uuid`
                pub mod uuid {
                    use super::*;

                    /// Path parameters for `POST /api/remote/servers/:uuid`
                    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                    pub struct Path {
                        pub uuid: Uuid,
                    }

                    /// `POST /api/remote/servers/:uuid/install`
                    pub mod install {
                        use super::*;

                        /// Body for `POST /api/remote/servers/:uuid/install`
                        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                        pub struct Body {
                            successful: bool,
                            reinstall: bool,
                        }

                        /// Response for `POST /api/remote/servers/:uuid/install`
                        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                        pub struct Response;
                    }
                }
            }

            /// `POST /api/remote/activity`
            pub mod activity {
                use super::*;

                /// Body for `POST /api/remote/activity`
                #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                pub struct Body {
                    pub data: Vec<body::Data>,
                }

                /// Body data for `POST /api/remote/activity`
                pub mod body {
                    use super::*;

                    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                    pub struct Data {
                        pub server: String,
                        pub event: String,
                        pub timestamp: String,
                        pub metadata: serde_json::Map<String, serde_json::Value>,
                        pub ip: String,
                        pub user: String,
                    }
                }

                /// `POST /api/remote/activity`
                #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                pub struct Response;
            }
        }
    }
}
