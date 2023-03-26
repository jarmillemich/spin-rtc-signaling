#![allow(dead_code, unused)]

use std::marker::PhantomData;

use anyhow::{anyhow, Result};
use rand::{distributions::Alphanumeric, Rng};
use serde_json::{json, Value};
use spin_sdk::{
    redis::{self, RedisParameter, RedisResult},
};

// TODO LATER Apparently Spin has an integrated SQL-like KV store, maybe use that.
//            However, implicit data expiration is nice...

const REDIS_ADDRESS_ENV: &str = "REDIS_ADDRESS";

pub struct RedisHelper(PhantomData<()>);

// General/test stuff
impl RedisHelper {
    fn address() -> Result<String> {
        // Unfortunately there doesn't seem to be a way to include this with env variables without it ending up in VC?
        //std::env::var(REDIS_ADDRESS_ENV).map_err(|_| anyhow!("Failed to get redis connection"))

        // Just embedding for now
        Ok(String::from(include_str!("../redis.env")))
    }

    /// Retrieve our test value from Redis, to test connectivity
    pub fn get_test_value() -> Result<u32> {
        match redis::get(&Self::address()?, "test") {
            Ok(value) => {
                if value.is_empty() { Ok(0u32) }
                else { Ok(std::str::from_utf8(value.as_slice())?.parse::<u32>()?) }
            },
            _ => {
                println!("New sequence");
                Ok(0u32)
            }
        }
    }

    /// Update our test value in Redis, to test connectivity
    pub fn set_test_value(val: u32) -> Result<()> {
        redis::set(&Self::address()?, "test", val.to_string().as_bytes()).map_err(|_| {
            anyhow!("Failed to update value")
        })
    }

    /// Wrapper for redis::execute
    fn execute(command: &str, arguments: &[RedisParameter]) -> Result<Vec<RedisResult>> {
        // TODO wrap RedisParameter so we can just pass in String like a sane person instead of encoding it everywhere
        redis::execute(&Self::address()?, command, arguments)
            .map_err(|_| anyhow!("Command failed: {command}"))
    }

    /// Generates a secret token for e.g. authentication
    fn generate_secret() -> String {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect()
    }
}

// Session management
impl RedisHelper {
    pub fn has_session(session_name: &str) -> Result<bool> {
        let key = RedisParameter::Binary("sessions".as_bytes());
        let name = RedisParameter::Binary(session_name.as_bytes());
        let res = Self::execute("HEXISTS", &[key, name]).map_err(|_| anyhow!("Error retrieving session"))?;

        let res = res.first().ok_or_else(|| anyhow!("Error retrieving session"))?;

        match res {
            RedisResult::Int64(1) => Ok(true),
            RedisResult::Int64(0) => Ok(false),
            _ => Err(anyhow!("Error retrieving session")),
        }
    }

    /// Registers a new session and returns the host's authentication secret
    pub fn register_session(session_name: &str, is_public: bool, host_name: &str) -> Result<String> {
        // TODO remove they key if we fail anywhere here?
        let host_secret = Self::generate_secret();
        
        Self::set_session_property(session_name, "public", RedisParameter::Int64(is_public as i64))?;
        Self::set_session_property(session_name, "host_name", RedisParameter::Binary(host_name.as_bytes()))?;
        Self::set_session_property(session_name, "host_secret", RedisParameter::Binary(host_secret.as_bytes()))?;
        Self::set_session_expire(session_name, 600)?;
        
        Ok(host_secret)
    }

    fn set_session_property(session_name: &str, field: &str, value: RedisParameter) -> Result<()> {
        let key = format!("sessions:{session_name}");
        let key = RedisParameter::Binary(key.as_bytes());
        let field = RedisParameter::Binary(field.as_bytes());

        Self::execute("HSET", &[key, field, value]).map_err(|_| anyhow!("Failed to set session property"))?;

        Ok(())
    }

    fn set_session_expire(session_name: &str, seconds: i64) -> Result<()> {
        let key = format!("sessions:{session_name}");
        let key = RedisParameter::Binary(key.as_bytes());
        let seconds = RedisParameter::Int64(seconds);

        Self::execute("EXPIRE", &[key, seconds]).map_err(|_| anyhow!("Failed to set session expiration"))?;

        Ok(())
    }

    /// Determine if the given secret is correct for the host of session_name
    pub fn authenticate_host_message(session_name: &str, host_secret: &str) -> Result<bool> {
        let actual_secret = Self::get_host_secret(session_name)?;

        // Valid if we have a secret for this client and it matches the supplied value
        match actual_secret {
            Some(actual_secret) => {
                if host_secret != actual_secret {
                    println!("(Mismatch) Auth host got {host_secret} expecting {actual_secret}");
                }

                Ok(actual_secret == host_secret)
            },
            None => {
                println!("No host secret, got {host_secret}");
                Ok(false)
            },
        }
    }

    /// Gets the host's secret for a session
    fn get_host_secret(session_name: &str) -> Result<Option<String>> {
        let key = format!("sessions:{session_name}");
        println!("{key} host_secret");
        let key = RedisParameter::Binary(key.as_bytes());

        let field = RedisParameter::Binary("host_secret".as_bytes());

        let res = Self::execute("HGET", &[key, field])
            .map_err(|_| anyhow!("Error retrieving host secret 1"))?;

        if res.is_empty() {
            println!("Empty res get host secret {}", res.len());
            return Ok(None);
        }

        let secret = res.first().ok_or_else(|| anyhow!("Error retrieving host secret 2"))?;

        match secret {
            RedisResult::Binary(val) => {
                let decoded = std::str::from_utf8(val).map_err(|_| anyhow!("Error decoding host secret 3"))?;
                Ok(Some(decoded.into()))
            },
            RedisResult::Nil => Ok(None),
            _ => Err(anyhow!("Error decoding host secret 4")),
        }
    }
}

// Connection request management
impl RedisHelper {
    /// Determines if the given secret is correct for this client_name joining session_name
    pub fn authenticate_client_message(session_name: &str, client_name: &str, client_secret: &str) -> Result<bool> {
        let actual_secret = Self::get_client_secret(session_name, client_name)?;

        // Valid if we have a secret for this client and it matches the supplied value
        match actual_secret {
            Some(actual_secret) => Ok(actual_secret == client_secret),
            None => Ok(false),
        }
    }

    /// Generates a secret for a client to join a session with
    fn register_client_secret(session_name: &str, client_name: &str) -> Result<String> {
        let secret = Self::generate_secret();
        
        // Save to store
        let key = format!("sessions:{session_name}:clients:{client_name}");
        let key = RedisParameter::Binary(key.as_bytes());

        // Expire after a while
        let secret_parameter = RedisParameter::Binary(secret.as_bytes());
        let ex = RedisParameter::Binary("EX".as_bytes());
        let expire_seconds = RedisParameter::Int64(600);

        let res = Self::execute("SET", &[key, secret_parameter, ex, expire_seconds]);

        Ok(secret)
    }

    /// Retrieves the secret for the specified client
    fn get_client_secret(session_name: &str, client_name: &str) -> Result<Option<String>> {
        let key = format!("sessions:{session_name}:clients:{client_name}");
        let key = RedisParameter::Binary(key.as_bytes());
        let res = Self::execute("GET", &[key]).map_err(|_| anyhow!("Error retrieving client secret"))?;

        match res.first() {
            Some(RedisResult::Binary(val)) => {
                let decoded = std::str::from_utf8(val).map_err(|_| anyhow!("Error decoding client secret"))?;
                Ok(Some(decoded.into()))
            },
            // One of these is correct...
            Some(RedisResult::Nil) => Ok(None),
            None => Ok(None),
            _ => Err(anyhow!("Error decoding client secret")),
        }
    }

    /// Does some session already have this client?
    pub fn session_has_client(session_name: &str, client_name: &str) -> Result<bool> {
        // Present if we have a secret registered for them
        Ok(Self::get_client_secret(session_name, client_name)?.is_some())
    }

    /// Initiates a client joining a session, returns their secret
    pub fn initiate_join(session_name: &str, client_name: &str, rtc_offer: &str) -> Result<String> {
        if Self::session_has_client(session_name, client_name)? {
            return Err(anyhow!("Name already taken"));
        }

        // Forward to the session host
        Self::push_message_to_host(session_name, json!({
            "type": "start_join",
            "client_name": client_name,
            "client_offer": rtc_offer
        }))?;

        Self::register_client_secret(session_name, client_name)
    }

    /// Send one or more ice candidates from a client to a host
    /// Assumes we are already authenticated
    pub fn client_ice_candidate(session_name: &str, client_name: &str, candidates: Vec<&str>) -> Result<()> {
        Self::push_message_to_host(session_name, json!({
            "type": "ice_candidate",
            "client_name": client_name,
            "candidates": candidates
        }))
    }

    /// Send one or more ice candidates from a host to a client
    /// Assumes we are already authenticated
    pub fn host_ice_candidate(session_name: &str, client_name: &str, candidates: Vec<String>) -> Result<()> {
        Self::push_message_to_client(session_name, client_name, &json!({
            "type": "ice_candidate",
            "candidates": candidates
        }))
    }

    /// Adds a message to the host's message queue/mailbox
    fn push_message_to_host(session_name: &str, message: Value) -> Result<()> {
        let message = message.to_string();
        
        let key = format!("sessions:{session_name}:message_queue");
        let key = RedisParameter::Binary(key.as_bytes());
        let message = RedisParameter::Binary(message.as_bytes());

        Self::execute("LPUSH", &[key.clone(), message]).map_err(|e| anyhow!("Failed to enqueue message"))?;
        Self::execute("EXPIRE", &[key.clone(), RedisParameter::Int64(600)]);

        Ok(())
    }

    pub fn get_messages_for_host(session_name: &str) -> Result<Vec<String>> {
        let key = format!("sessions:{session_name}:message_queue");
        Self::read_message_queue(key)
    }

    /// Adds a message to the host's message queue/mailbox
    pub fn push_message_to_client(session_name: &str, client_name: &str, message: &Value) -> Result<()> {
        let message = message.to_string();
        
        let key = format!("sessions:{session_name}:message_queue:{client_name}");
        let key = RedisParameter::Binary(key.as_bytes());
        let message = RedisParameter::Binary(message.as_bytes());

        Self::execute("LPUSH", &[key, message]).map_err(|e| anyhow!("Failed to enqueue message"))?;

        Ok(())
    }

    
    
    pub fn get_messages_for_client(session_name: &str, client_name: &str) -> Result<Vec<String>> {
        let key = format!("sessions:{session_name}:message_queue:{client_name}");
        Self::read_message_queue(key)
    }

    /// Takes a bunch of messages from the specified queue and returns them as Strings
    fn read_message_queue_future(key: String) -> Result<Vec<String>> {
        // TODO if we fail to decode the messages will still be removed from the queue. Which, is probably fine anyway
        // Alas, BLMPOP is not supported on our current substrate (6.2), use the other one

        let key = RedisParameter::Binary(key.as_bytes());

        let count = RedisParameter::Binary("COUNT".as_bytes());

        let res = Self::execute("BLMPOP", &[
            RedisParameter::Int64(5), // 5 second timeout
            RedisParameter::Int64(1), // 1 key to look at
            key,
            count,
            RedisParameter::Int64(10), // Get at most 10 messages
        ])?;

        if res[0] == RedisResult::Nil {
            // No results
            return Ok(Vec::new());
        }

        let ret: Result<Vec<String>> = match (res[0]) {
            // No results
            RedisResult::Nil => Ok(Vec::new()),
            // Some results
            RedisResult::Binary(_) => res.iter().skip(1).map(|r| match r {
                RedisResult::Binary(message) =>
                    std::str::from_utf8(message)
                        .map(String::from)
                        .map_err(|_| anyhow!("Invalid message format")),
                _ => Err(anyhow!("Invalid message format"))
            }).collect(),
            _ => Err(anyhow!("Invalid message response"))
        };
        
        ret
    }

    fn read_message_queue(key: String) -> Result<Vec<String>> {
        let key = RedisParameter::Binary(key.as_bytes());
        let timeout_seconds = RedisParameter::Int64(5);
        let args = [key, timeout_seconds];

        
        let mut at = Self::execute("BLPOP", &args[..])?;
        let mut ret = Vec::new();

        while let Some(RedisResult::Binary(_)) = at.first() {
            match at.last() {
                Some(RedisResult::Binary(item)) => {
                    ret.push(std::str::from_utf8(item)?.into());
                },
                _ => return Err(anyhow!("Unexpected message format")),
            }

            at = Self::execute("LPOP", &args[0..1])?
        };

        Ok(ret)
    }
}