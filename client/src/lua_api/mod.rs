mod player;
mod world;
pub mod render;
mod packet;
pub mod item;
pub mod http;

use anyhow::Result;
use mlua::prelude::*;
use std::sync::OnceLock;
use std::time::Instant;

static WALL_CLOCK_START: OnceLock<Instant> = OnceLock::new();

/// Registers the full `mc` and `anemoia` globals into the Lua state.
pub fn register(lua: &Lua) -> Result<()> {
    let mc = lua.create_table()?;
    player::register(lua, &mc)?;
    world::register(lua, &mc)?;
    lua.globals().set("mc", &mc)?;

    let anemoia = lua.create_table()?;
    let modules = lua.create_table()?;
    anemoia.set("_modules", modules)?;
    render::register(lua, &anemoia)?;
    packet::register(lua, &anemoia)?;
    http::register(lua, &anemoia)?;

    anemoia.set(
        "register",
        lua.create_function(|lua, module: LuaTable| {
            let anemoia: LuaTable = lua.globals().get("anemoia")?;
            let list: LuaTable = anemoia.get("_modules")?;
            list.push(module)?;
            Ok(())
        })?,
    )?;

    mc.set(
        "chat",
        lua.create_function(|_, message: String| {
            with_env(|env| {
                let mc_obj = crate::mc::minecraft::Minecraft::get_instance(env)?
                    .ok_or_else(|| anyhow::anyhow!("Minecraft not ready"))?;
                let player = crate::mc::player::LocalPlayer::from_minecraft(&mc_obj, env)?
                    .ok_or_else(|| anyhow::anyhow!("Player is null"))?;
                player.send_chat(env, &message)
            })
        })?,
    )?;

    mc.set(
        "click",
        lua.create_function(|_, ()| {
            let jvm = crate::jvm::Jvm::get();
            let mut env = jvm
                .attach()
                .map_err(|e| LuaError::runtime(e.to_string()))?;

            let mc_obj = crate::mc::minecraft::Minecraft::get_instance(&mut env)
                .map_err(|e| LuaError::runtime(e.to_string()))?
                .ok_or_else(|| LuaError::runtime("Minecraft not ready"))?;

            mc_obj.left_click_mouse(&mut env).map_err(|e| LuaError::runtime(e.to_string()))
        })?,
    )?;

    mc.set(
        "use_item",
        lua.create_function(|_, hand: Option<String>| {
            with_env(|env| {
                let mc_obj = crate::mc::minecraft::Minecraft::get_instance(env)?
                    .ok_or_else(|| anyhow::anyhow!("Minecraft not ready"))?;
                mc_obj.use_item(env, &hand.unwrap_or("MAIN_HAND".to_owned()))
            })
        })?,
    )?;

    mc.set(
        "right_click",
        lua.create_function(|_, ()| {
            let jvm = crate::jvm::Jvm::get();
            let mut env = jvm
                .attach()
                .map_err(|e| LuaError::runtime(e.to_string()))?;

            let mc_obj = crate::mc::minecraft::Minecraft::get_instance(&mut env)
                .map_err(|e| LuaError::runtime(e.to_string()))?
                .ok_or_else(|| LuaError::runtime("Minecraft not ready"))?;

            mc_obj.right_click_mouse(&mut env).map_err(|e| LuaError::runtime(e.to_string()))
        })?,
    )?;

    mc.set(
        "send_packet",
        lua.create_function(|_, (packet_ud, _raw): (LuaAnyUserData, Option<bool>)| {
            let packet_ref = packet_ud.borrow::<crate::mc::packet::Packet>()?;

            let jvm = crate::jvm::Jvm::get();
            let mut env = jvm.attach().map_err(|e| LuaError::runtime(e.to_string()))?;

            let mc_obj = crate::mc::minecraft::Minecraft::get_instance(&mut env)
                .map_err(|e| LuaError::runtime(e.to_string()))?
                .ok_or_else(|| LuaError::runtime("Minecraft not ready"))?;

            let player = crate::mc::player::LocalPlayer::from_minecraft(&mc_obj, &mut env)
                .map_err(|e| LuaError::runtime(e.to_string()))?
                .ok_or_else(|| LuaError::runtime("Player is null"))?;

            let connection = player.get_connection(&mut env)
                .map_err(|e| LuaError::runtime(e.to_string()))?;

            // Always bypass on_packet_send hook — on_tick holds the LUA mutex and
            // re-acquiring it here deadlocks the game. The raw param is kept for
            // script compatibility but has no effect.
            connection.send(&mut env, &packet_ref, false).map_err(|e| LuaError::runtime(e.to_string()))
        })?,
    )?;

    mc.set(
        "is_key_down",
        lua.create_function(|_, key: i32| {
            // Both cached after first call — no dlsym or JNI on subsequent calls
            let glfw = crate::glfw::Glfw::get()
                .map_err(|e| LuaError::runtime(e.to_string()))?;

            let win = {
                let ptr = crate::mc::window::cached_window_ptr();
                if !ptr.is_null() {
                    ptr
                } else {
                    let jvm = crate::jvm::Jvm::get();
                    let mut env = jvm.attach().map_err(|e| LuaError::runtime(e.to_string()))?;
                    let mc_obj = crate::mc::minecraft::Minecraft::get_instance(&mut env)
                        .map_err(|e| LuaError::runtime(e.to_string()))?
                        .ok_or_else(|| LuaError::runtime("Minecraft not ready"))?;
                    crate::mc::window::get_glfw_window_cached(&mc_obj, &mut env)
                        .map_err(|e| LuaError::runtime(e.to_string()))?
                }
            };

            // GLFW mouse buttons occupy codes 0-7; keyboard starts at 32 (SPACE).
            if key <= 7 {
                Ok(glfw.mouse_button_pressed(win, key))
            } else {
                Ok(glfw.key_pressed(win, key))
            }
        })?,
    )?;

    mc.set(
        "right_click_delay",
        lua.create_function(|_, ()| {
            with_env(|env| {
                let mc_obj = crate::mc::minecraft::Minecraft::get_instance(env)?
                    .ok_or_else(|| anyhow::anyhow!("Minecraft not ready"))?;
                mc_obj.get_right_click_delay_timer(env)
            })
        })?,
    )?;

    mc.set(
        "set_right_click_delay",
        lua.create_function(|_, delay: i32| {
            with_env(|env| {
                let mc_obj = crate::mc::minecraft::Minecraft::get_instance(env)?
                    .ok_or_else(|| anyhow::anyhow!("Minecraft not ready"))?;
                mc_obj.set_right_click_delay_timer(env, delay)
            })
        })?,
    )?;

    mc.set(
        "place_block",
        lua.create_function(|_, (x, y, z, face): (i32, i32, i32, String)| {
            with_env(|env| {
                let mc_obj = crate::mc::minecraft::Minecraft::get_instance(env)?
                    .ok_or_else(|| anyhow::anyhow!("Minecraft not ready"))?;
                mc_obj.place_block_on_face(env, x, y, z, &face)
            })
        })?,
    )?;

    mc.set(
        "set_gamma",
        lua.create_function(|_, value: f64| {
            with_env(|env| {
                let mc_obj = crate::mc::minecraft::Minecraft::get_instance(env)?
                    .ok_or_else(|| anyhow::anyhow!("Minecraft not ready"))?;
                mc_obj.set_gamma(env, value)
            })
        })?,
    )?;

    mc.set(
        "fov",
        lua.create_function(|_, ()| {
            with_env(|env| {
                let mc_obj = crate::mc::minecraft::Minecraft::get_instance(env)?
                    .ok_or_else(|| anyhow::anyhow!("Minecraft not ready"))?;
                let options = env.get_field(mc_obj.jni_ref.as_obj(), "options",
                    "Lnet/minecraft/client/Options;")?.l()?;
                let fov_opt = env.get_field(&options, "fov",
                    "Lnet/minecraft/client/OptionInstance;")?.l()?;
                let boxed = env.call_method(&fov_opt, "get", "()Ljava/lang/Object;", &[])?.l()?;
                Ok(env.call_method(&boxed, "doubleValue", "()D", &[])?.d()? as f32)
            })
        })?,
    )?;

    mc.set(
        "clock",
        lua.create_function(|_, ()| {
            Ok(WALL_CLOCK_START.get_or_init(Instant::now).elapsed().as_secs_f64())
        })?,
    )?;

    mc.set(
        "freeze_pos",
        lua.create_function(|_, ()| {
            crate::mc::netty::set_freeze(true);
            Ok(())
        })?,
    )?;

    mc.set(
        "unfreeze_pos",
        lua.create_function(|_, ()| {
            crate::mc::netty::set_freeze(false);
            Ok(())
        })?,
    )?;

    mc.set(
        "inventory_click",
        lua.create_function(|_, (container_id, slot, button, click_type): (i32, i32, i32, String)| {
            with_env(|env| {
                let mc_obj = crate::mc::minecraft::Minecraft::get_instance(env)?
                    .ok_or_else(|| anyhow::anyhow!("Minecraft not ready"))?;
                mc_obj.inventory_click(env, container_id, slot, button, &click_type)
            })
        })?,
    )?;

    anemoia.set(
        "zulip_config",
        lua.create_function(|_, cfg: LuaTable| {
            let mut config = crate::zulip::get_config();
            if let Ok(enabled) = cfg.get::<bool>("enabled") { config.enabled = enabled; }
            if let Ok(url) = cfg.get::<String>("url") { config.url = url; }
            if let Ok(email) = cfg.get::<String>("email") { config.email = email; }
            if let Ok(api_key) = cfg.get::<String>("api_key") { config.api_key = api_key; }
            if let Ok(stream) = cfg.get::<String>("stream") { config.stream = stream; }
            if let Ok(topic) = cfg.get::<String>("topic") { config.topic = topic; }
            if let Ok(poll_rate) = cfg.get::<f64>("poll_rate") { config.poll_rate = poll_rate; }
            
            crate::zulip::set_config(config);
            Ok(())
        })?,
    )?;

    anemoia.set(
        "zulip_send",
        lua.create_function(|_, msg: String| {
            crate::zulip::send_message(msg);
            Ok(())
        })?,
    )?;

    anemoia.set(
        "zulip_get_messages",
        lua.create_function(|lua, ()| {
            let messages = crate::zulip::get_messages();
            let table = lua.create_table()?;
            for (i, msg) in messages.into_iter().enumerate() {
                let msg_table = lua.create_table()?;
                msg_table.set("sender", msg.sender)?;
                msg_table.set("content", msg.content)?;
                msg_table.set("time", msg.time)?;
                msg_table.set("stream", msg.stream)?;
                msg_table.set("topic", msg.topic)?;
                table.set(i + 1, msg_table)?;
            }
            Ok(table)
        })?,
    )?;

    anemoia.set(
        "zulip_clear",
        lua.create_function(|_, ()| {
            crate::zulip::clear_messages();
            Ok(())
        })?,
    )?;

    lua.globals().set("anemoia", anemoia)?;
    Ok(())
}

fn with_env<F, T>(f: F) -> LuaResult<T>
where
    F: FnOnce(&mut jni::JNIEnv) -> anyhow::Result<T>,
{
    let mut env = crate::jvm::Jvm::get()
        .attach()
        .map_err(|e| LuaError::runtime(e.to_string()))?;
    f(&mut env).map_err(|e| {
        let _ = env.exception_clear();
        LuaError::runtime(e.to_string())
    })
}
