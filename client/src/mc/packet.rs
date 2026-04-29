use anyhow::Result;
use jni::{objects::GlobalRef, JNIEnv};
use mlua::prelude::*;

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
    }
}

pub struct Connection {
    pub jni_ref: GlobalRef,
}

impl Connection {
    pub fn send(&self, env: &mut JNIEnv, packet: &Packet, use_hook: bool) -> Result<()> {
        if use_hook {
            // Trigger Lua event
            let jni_ref = env.new_global_ref(packet.jni_ref.as_obj())?;
            let p_obj = Packet::new(jni_ref);
            
            let cancelled = crate::lua_engine::on_packet_send(p_obj)?;
            if cancelled {
                return Ok(());
            }
        }

        env.call_method(
            self.jni_ref.as_obj(),
            "send",
            "(Lnet/minecraft/network/protocol/Packet;)V",
            &[jni::objects::JValue::Object(packet.jni_ref.as_obj())],
        )?;
        Ok(())
    }
}
