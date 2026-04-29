use mlua::prelude::*;
use ureq;
use std::collections::HashMap;
use base64::{Engine as _, engine::general_purpose};

pub fn register(lua: &Lua, anemoia: &LuaTable) -> anyhow::Result<()> {
    anemoia.set("http", lua.create_function(|lua, (url, options): (String, Option<LuaTable>)| {
        let mut method = "GET";
        let mut headers = HashMap::new();
        let mut body: Option<String> = None;

        if let Some(opts) = options {
            if let Ok(m) = opts.get::<String>("method") {
                // Keep the string owned so we can use its ref
                let method_owned = m.to_uppercase();
                method = match method_owned.as_str() {
                    "POST" => "POST",
                    "PUT" => "PUT",
                    "DELETE" => "DELETE",
                    _ => "GET",
                };
            }
            if let Ok(h) = opts.get::<LuaTable>("headers") {
                for pair in h.pairs::<String, String>() {
                    if let Ok((k, v)) = pair {
                        headers.insert(k, v);
                    }
                }
            }
            if let Ok(b) = opts.get::<String>("body") {
                body = Some(b);
            }
        }

        let mut req = match method {
            "POST" => ureq::post(&url),
            "PUT" => ureq::put(&url),
            "DELETE" => ureq::delete(&url),
            _ => ureq::get(&url),
        };

        for (k, v) in headers {
            req = req.set(&k, &v);
        }

        let resp = if let Some(b) = body {
            req.send_string(&b)
        } else {
            req.call()
        };

        match resp {
            Ok(r) => {
                let status = r.status();
                let body_str = r.into_string().unwrap_or_default();
                let res = lua.create_table()?;
                res.set("status", status)?;
                res.set("body", body_str.clone())?;
                
                // Try to parse JSON
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&body_str) {
                    res.set("json", lua_from_json(lua, json_val)?)?;
                }
                
                Ok(res)
            }
            Err(e) => {
                let res = lua.create_table()?;
                res.set("error", e.to_string())?;
                Ok(res)
            }
        }
    })?)?;

    anemoia.set("base64_encode", lua.create_function(|_, data: String| {
        Ok(general_purpose::STANDARD.encode(data))
    })?)?;

    Ok(())
}

fn lua_from_json(lua: &Lua, value: serde_json::Value) -> LuaResult<LuaValue> {
    match value {
        serde_json::Value::Null => Ok(LuaValue::Nil),
        serde_json::Value::Bool(b) => Ok(LuaValue::Boolean(b)),
        serde_json::Value::Number(n) => Ok(LuaValue::Number(n.as_f64().unwrap_or(0.0))),
        serde_json::Value::String(s) => Ok(lua.create_string(s)?.into_lua(lua)?),
        serde_json::Value::Array(arr) => {
            let t = lua.create_table()?;
            for (i, v) in arr.into_iter().enumerate() {
                t.set(i + 1, lua_from_json(lua, v)?)?;
            }
            Ok(LuaValue::Table(t))
        }
        serde_json::Value::Object(obj) => {
            let t = lua.create_table()?;
            for (k, v) in obj {
                t.set(k, lua_from_json(lua, v)?)?;
            }
            Ok(LuaValue::Table(t))
        }
    }
}
