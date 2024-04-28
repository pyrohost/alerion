use std::sync::Arc;

use poem::http::StatusCode;
use poem::listener::TcpListener;
use poem::web::{Data, Json, Path, Query};
use poem::{get, handler, post, EndpointExt as _, IntoResponse, Response, Route, Server};

pub mod datamodel;

pub struct State {
    pub servers: Vec<crate::datamodel::get::api::remote::servers::response::Data>,
    pub path: String,
}

impl State {
    pub fn new(path: String) -> Self {
        Self {
            servers: Vec::new(),
            path,
        }
    }
}

#[handler]
async fn get_remote_servers(
    state: Data<&Arc<State>>,
    query: Query<datamodel::get::api::remote::servers::QueryParams>,
) -> Response {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(10);

    if page < 1 || per_page < 1 {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let last_page = state.servers.len() as u32 / per_page + 1;

    let links = (0..last_page)
        .map(
            |i| crate::datamodel::get::api::remote::servers::response::meta::Link {
                url: Some(format!(
                    "{}/api/remote/servers?page={}&per_page={}",
                    state.path, i, per_page
                )),
                label: i.to_string(),
                active: i == page,
            },
        )
        .collect::<Vec<crate::datamodel::get::api::remote::servers::response::meta::Link>>();

    let links = vec![
        crate::datamodel::get::api::remote::servers::response::meta::Link {
            url: None,
            label: "&laquo; Previous".to_owned(),
            active: false,
        },
    ]
    .into_iter()
    .chain(links)
    .chain(
        vec![
            crate::datamodel::get::api::remote::servers::response::meta::Link {
                url: None,
                label: "Next &raquo;".to_owned(),
                active: false,
            },
        ]
        .into_iter(),
    )
    .collect::<Vec<crate::datamodel::get::api::remote::servers::response::meta::Link>>();

    let res = crate::datamodel::get::api::remote::servers::Response {
        data: state
            .servers
            .clone()
            .windows(per_page as usize)
            .nth(page as usize - 1)
            .unwrap_or_default()
            .to_vec(),
        meta: crate::datamodel::get::api::remote::servers::response::Meta {
            current_page: page as i64,
            from: 1,
            last_page: last_page as i64,
            links,
            path: state.path.clone(),
            per_page: per_page as i64,
            to: 1,
            total: state.servers.len() as i64,
        },
        links: crate::datamodel::get::api::remote::servers::response::Links {
            prev: if page > 1 {
                Some(format!(
                    "{}/api/remote/servers?page={}&per_page={}",
                    state.path,
                    page - 1,
                    per_page
                ))
            } else {
                None
            },
            next: if state.servers.len() as u32 > page * per_page {
                Some(format!(
                    "{}/api/remote/servers?page={}&per_page={}",
                    state.path,
                    page + 1,
                    per_page
                ))
            } else {
                None
            },
            first: format!(
                "{}/api/remote/servers?page=1&per_page={}",
                state.path, per_page
            ),
            last: format!(
                "{}/api/remote/servers?page={}&per_page={}",
                state.path, last_page, per_page
            ),
        },
    };

    Json(res).into_response()
}

#[handler]
async fn get_remote_server(
    path: Path<crate::datamodel::get::api::remote::servers::uuid::Path>,
    state: Data<&Arc<State>>,
) -> Response {
    return match state.servers.iter().find(|s| s.uuid == path.uuid) {
        Some(server) => Json(server).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    };
}

#[handler]
async fn get_remote_server_installations(
    path: Path<crate::datamodel::get::api::remote::servers::uuid::Path>,
    state: Data<&Arc<State>>,
) -> Response {
    let server = state.servers.iter().find(|s| s.uuid == path.uuid);
    if server.is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let server = server.expect("server not found").clone();

    let res = crate::datamodel::get::api::remote::servers::uuid::install::Response {
        container_image: server.settings.container.image,
        entrypoint: "bash".to_owned(),
        script: server.settings.invocation,
    };

    Json(res).into_response()
}

#[handler]
async fn reset_remote_servers() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

#[handler]
async fn add_remote_server_activity(
    body: Json<datamodel::post::api::remote::activity::Body>,
    state: Data<&Arc<State>>,
) -> Response {
    for server in body.data.iter() {
        let server = state
            .servers
            .iter()
            .find(|s| s.uuid == uuid::Uuid::parse_str(&server.server).unwrap_or_default());
        if server.is_none() {
            return StatusCode::UNPROCESSABLE_ENTITY.into_response();
        }
    }

    StatusCode::OK.into_response()
}

#[handler]
async fn update_remote_server_installation(
    path: Path<crate::datamodel::get::api::remote::servers::uuid::Path>,
    _body: Json<crate::datamodel::post::api::remote::servers::uuid::install::Body>,
    state: Data<&Arc<State>>,
) -> Response {
    let server = state.servers.iter().find(|s| s.uuid == path.uuid);
    if server.is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }

    StatusCode::NO_CONTENT.into_response()
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let state = Arc::new(State::new("localhost".to_owned()));

    let app = Route::new()
        .at("/api/remote/servers", get(get_remote_servers))
        .at("/api/remote/servers/reset", post(reset_remote_servers))
        .at("/api/remote/activity", post(add_remote_server_activity))
        .at("/api/remote/servers/:uuid", get(get_remote_server))
        .at(
            "/api/remote/servers/:uuid/install",
            get(get_remote_server_installations).post(update_remote_server_installation),
        )
        .data(state);

    Server::new(TcpListener::bind("localhost:3000"))
        .run(app)
        .await
}
