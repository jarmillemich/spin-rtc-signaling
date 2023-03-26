use anyhow::{anyhow, Result};
use http::Method;
use serde_json::{Value, json};
use spin_sdk::{
    http::{Request, Response},
    http_component,
};

mod redis_helper;
use redis_helper::RedisHelper;

mod req_helpers;
use req_helpers::*;

mod random_util;
use random_util::generate_name;


/// A simple Spin HTTP component.
#[http_component]
fn handle_rust_signaling(req: Request) -> Result<Response> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => Ok(http::Response::builder().status(200).body(Some(include_str!("./index.html").into()))?),
        (&Method::GET, "/test") => test_route(),

        // Start a session
        (&Method::POST, "/host") => post_host_session(req),
        // Receive messages from a client
        (&Method::GET, "/host/messages") => get_receive_host_messages(&req),
        
        (&Method::POST, "/join/response") => post_send_join_responses(&req),

        // Get the list of public sessions
        (&Method::GET, "/sessions") => get_session_list(&req),
        // Start joining a session
        (&Method::POST, "/join") => join_session(&req),
        // Send messages to the host
        (&Method::POST, "/join/candidates") => post_send_join_candidates(&req),
        // Receive messages from the host
        (&Method::GET, "/join/messages") => get_receive_join_responses(&req),
        

        _ => Ok(http::Response::builder().status(404).body(Some("Not found".into()))?)
    }
}

/*
    So, we want a couple routes to establish RTC connections:
    - One to advertise we have a session (public or private)
        - Requires public/private, RTC description
        - Returns session code/name
    - One to list public sessions
    - One to request to join a session
        - Might need two requests, one to get the host description and one to send ours?
        - Sends our RTC description to redis
        - Host will have to poll, since we're limited to 10 seconds
            - We could do long polling I suppose...
    - One for hosts to poll for joiners!
*/

fn post_host_session(req: Request) -> Result<Response> {
    // Retrieve variables
    let body = req.body().as_ref().ok_or_else(|| anyhow!("Invalid body"))?;
    let body = std::str::from_utf8(body).map_err(|_| anyhow!("Invalid body"))?;
    let body: Value = serde_json::from_str(body).map_err(|_| anyhow!("Invalid body"))?;

    let is_public = required_json_bool(&body, "public")?;
    let host_name = required_json_str(&body, "host_name")?;
    
    // Generate a name, ensuring we don't already have such a session
    let mut safety = 0;
    let session_name = loop {
        let ret = generate_name();
        if !RedisHelper::has_session(&ret)? {
            break ret;
        }

        safety += 1;
        if safety > 1000 {
            return Err(anyhow!("Failed to generate session name"));
        }
    };

    // Register the session
    let host_secret = RedisHelper::register_session(&session_name, is_public, host_name)
        .map_err(|_| anyhow!("Failed to register session"))?;

    // Return the session name to the requestor
    let res_body = json!({
        "success": true,
        "session_name": session_name,
        "host_secret": host_secret,
    });

    http::Response::builder()
        .status(200)
        .body(Some(res_body.to_string().into()))
        .map_err(|_| anyhow!("Failed to build response"))
}

fn get_receive_host_messages(req: &Request) -> Result<Response> {
    let query = parse_query(req)?;
    let session_name = required_query(&query, "session_name")?;
    let host_secret = required_query(&query, "host_secret")?;

    if !RedisHelper::authenticate_host_message(session_name, host_secret)? {
        return unauthenticated();
    }

    let messages = RedisHelper::get_messages_for_host(session_name)?;

    let res = json!(messages);
    http::Response::builder()
        .status(200)
        .body(Some(res.to_string().into()))
        .map_err(|_| anyhow!("Failed to build response"))
}

/// Send messages to a client
fn post_send_join_responses(req: &Request) -> Result<Response> {
    // Retrieve variables
    let body = get_json_body(req)?;
    
    let session_name = required_json_str(&body, "session_name")?;
    let client_name = required_json_str(&body, "client_name")?;
    let host_secret = required_json_str(&body, "host_secret")?;
    let messages = &body["messages"];

    if !RedisHelper::authenticate_host_message(session_name, host_secret)? {
        return unauthenticated();
    }

    RedisHelper::push_message_to_client(session_name, client_name, messages)?;

    http::Response::builder()
        .status(200)
        .body(None)
        .map_err(|_| anyhow!("Failed to build response"))
}

fn get_session_list(_req: &Request) -> Result<Response> {
    todo!()
}

/// A client is initiating the join process
fn join_session(req: &Request) -> Result<Response> {
    // Retrieve variables
    let body = get_json_body(req)?;
    
    let session_name = required_json_str(&body, "session_name")?;
    let client_name = required_json_str(&body, "client_name")?;
    let rtc_offer = required_json_str(&body, "rtc_offer")?;

    let client_secret = RedisHelper::initiate_join(session_name, client_name, rtc_offer)?;

    let res_body = json!({
        "success": true,
        "client_secret": client_secret,
    });

    http::Response::builder()
        .status(200)
        .body(Some(res_body.to_string().into()))
        .map_err(|_| anyhow!("Failed to build response"))
}

/// A client is sending a message to the host
fn post_send_join_candidates(req: &Request) -> Result<Response> {
    let body = get_json_body(req)?;
    
    let session_name = required_json_str(&body, "session_name")?;
    let client_name = required_json_str(&body, "client_name")?;
    let client_secret = required_json_str(&body, "client_secret")?;
    let candidates = &body["candidates"];

    let candidates = candidates.as_array()
        .map(|candidates| 
            candidates
                .iter()
                .map(|candidate| candidate.as_str())
                .collect::<Option<Vec<_>>>()
        )
        // TODO not have this be an Option<Option<_>>?
        .ok_or_else(|| anyhow!("Candidates must be an array of string canidates"))?
        .ok_or_else(|| anyhow!("Candidates must be an array of string canidates"))?;

    if !RedisHelper::authenticate_client_message(session_name, client_name, client_secret)? {
        return unauthenticated();
    }

    RedisHelper::client_ice_candidate(session_name, client_name, candidates)?;

    http::Response::builder()
        .status(200)
        .body(None)
        .map_err(|_| anyhow!("Failed to build response"))
}

fn get_receive_join_responses(req: &Request) -> Result<Response> {
    let query = parse_query(req)?;
    let session_name = required_query(&query, "session_name")?;
    let client_name = required_query(&query, "client_name")?;
    let client_secret = required_query(&query, "client_secret")?;

    if !RedisHelper::authenticate_client_message(session_name, client_name, client_secret)? {
        return unauthenticated();
    }

    let messages = RedisHelper::get_messages_for_client(session_name, client_name)?;

    http::Response::builder()
        .status(200)
        .body(Some(json!(messages).to_string().into()))
        .map_err(|_| anyhow!("Failed to build response"))
}

/// Just a route to test connecting to our backing store
fn test_route() -> Result<Response> {
    let count = RedisHelper::get_test_value()? + 1;
    RedisHelper::set_test_value(count)?;

    let name = generate_name();

    Ok(
        http::Response::builder()
            .status(200)
            .header("foo", "bar")
            .body(Some(format!("Hello, {name} #{count}").into()))?
    )
}


fn unauthenticated() -> Result<Response> {
    http::Response::builder()
        .status(401)
        .body(None)
        .map_err(|_| anyhow!("Failed to build response"))
}

fn get_json_body(req: &Request) -> Result<Value> {
    // Retrieve variables
    let body = req.body().as_ref().ok_or_else(|| anyhow!("Invalid body"))?;
    let body = std::str::from_utf8(body).map_err(|_| anyhow!("Invalid body"))?;
    serde_json::from_str(body).map_err(|_| anyhow!("Invalid body"))
}