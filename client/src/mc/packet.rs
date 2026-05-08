use anyhow::Result;
use jni::{objects::GlobalRef, JNIEnv};
use mlua::prelude::*;
use std::sync::atomic::{AtomicI32, Ordering};

// Tracks how many packets are "in flight" from Lua sends.
// Incremented in Connection::send(), decremented in on_outgoing_native.
// Prevents double-hooking packets sent by mc.send_packet().
static LUA_SEND_INFLIGHT: AtomicI32 = AtomicI32::new(0);

pub fn mark_lua_send() {
    LUA_SEND_INFLIGHT.fetch_add(1, Ordering::Relaxed);
}

pub fn is_lua_send(_env: &mut jni::JNIEnv, _obj: &jni::objects::JObject) -> bool {
    let n = LUA_SEND_INFLIGHT.load(Ordering::Relaxed);
    if n > 0 {
        LUA_SEND_INFLIGHT.fetch_sub(1, Ordering::Relaxed);
        return true;
    }
    false
}

pub struct Packet {
    pub jni_ref: GlobalRef,
}

impl Packet {
    pub fn new(jni_ref: GlobalRef) -> Self {
        Self { jni_ref }
    }

    pub fn type_name(&self, env: &mut JNIEnv) -> Result<String> {
        let cls = env.get_object_class(self.jni_ref.as_obj())?;
        let name_jstr = env.call_method(&cls, "getName", "()Ljava/lang/String;", &[])?.l()?;
        let s: String = env.get_string((&name_jstr).into())?.into();
        Ok(s)
    }
}

impl LuaUserData for Packet {
    fn add_methods<M: LuaUserDataMethods<Self>>(m: &mut M) {
        m.add_method("type_name", |_, this, ()| {
            let jvm = crate::jvm::Jvm::get();
            let mut env = jvm.attach().map_err(|e| LuaError::runtime(e.to_string()))?;
            this.type_name(&mut env).map_err(|e| LuaError::runtime(e.to_string()))
        });

        m.add_method("fields", |lua, this, ()| {
            let jvm = crate::jvm::Jvm::get();
            let mut env = jvm.attach().map_err(|e| LuaError::runtime(e.to_string()))?;
            let fields = crate::mc::reflect::reflect_fields(&mut env, this.jni_ref.as_obj().as_raw())
                .map_err(|e| LuaError::runtime(e.to_string()))?;
            
            let table = lua.create_table()?;
            for (name, value) in fields {
                table.set(name, value)?;
            }
            Ok(table)
        });
    }
}

pub struct Connection {
    pub jni_ref: GlobalRef,
}

impl Connection {
    pub fn send(&self, env: &mut JNIEnv, packet: &Packet, use_hook: bool) -> Result<()> {
        let cancelled = if use_hook {
            let type_name = packet.type_name(env).unwrap_or_default();
            let hook_ref = env.new_global_ref(packet.jni_ref.as_obj())?;
            let cancelled = crate::lua_engine::on_packet_send(Packet::new(hook_ref))?;
            if let Ok(cap_ref) = env.new_global_ref(packet.jni_ref.as_obj()) {
                crate::packet_capture::push_out(type_name, cap_ref, cancelled);
            }
            cancelled
        } else {
            false
        };

        if cancelled {
            return Ok(());
        }

        // Mark as Lua-originated so on_outgoing_native skips the hook for this packet
        mark_lua_send();
        env.call_method(
            self.jni_ref.as_obj(),
            "send",
            "(Lnet/minecraft/network/protocol/Packet;)V",
            &[jni::objects::JValue::Object(packet.jni_ref.as_obj())],
        )?;
        Ok(())
    }
}
