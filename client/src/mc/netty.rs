use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// When set, the outgoing interceptor cancels all ServerboundMovePlayerPacket
/// variants without acquiring the Lua lock — eliminating the race condition
/// that lets camera-position packets slip through to the server during FreeCam.
pub static FREEZE_PACKETS: AtomicBool = AtomicBool::new(false);

pub fn set_freeze(active: bool) {
    FREEZE_PACKETS.store(active, Ordering::Release);
}

use anyhow::Result;
use jni::{objects::{JObject, JValue}, JNIEnv, NativeMethod};
use log::{info, warn};

use super::paths;

#[cfg(incoming_capture)]
const INTERCEPTOR_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/AnemoiaInterceptor.class"));

static INJECTED: AtomicBool = AtomicBool::new(false);
static ERRORS: AtomicU32 = AtomicU32::new(0);

/// Called every tick. Injects AnemoiaInterceptor into the Netty pipeline once
/// MC is connected to a server. No-ops after the first successful injection.
/// Gives up after 10 consecutive JNI errors (not "not connected yet" cases).
pub fn try_inject(env: &mut JNIEnv) {
    if INJECTED.load(Ordering::Relaxed) {
        return;
    }
    if ERRORS.load(Ordering::Relaxed) >= 10 {
        return;
    }

    #[cfg(incoming_capture)]
    match inject_inner(env) {
        Ok(true) => {
            INJECTED.store(true, Ordering::Relaxed);
            info!("AnemoiaInterceptor injected into Netty pipeline");
        }
        Ok(false) => {}
        Err(e) => {
            let _ = env.exception_clear();
            let n = ERRORS.fetch_add(1, Ordering::Relaxed);
            if n < 3 {
                warn!("Netty inject: {:#}", e);
            }
        }
    }
}

#[cfg(incoming_capture)]
fn inject_inner(env: &mut JNIEnv) -> Result<bool> {
    // Minecraft.getInstance()
    let mc_cls = crate::jvm::Jvm::find_class(env, paths::MINECRAFT)?;
    let mc = env
        .call_static_method(
            &mc_cls,
            "getInstance",
            "()Lnet/minecraft/client/Minecraft;",
            &[],
        )?
        .l()?;
    if mc.is_null() {
        return Ok(false);
    }

    // Minecraft.getConnection() → ClientPacketListener (null if not in-game)
    let listener = env
        .call_method(
            &mc,
            "getConnection",
            "()Lnet/minecraft/client/multiplayer/ClientPacketListener;",
            &[],
        )
        .and_then(|v| v.l())
        .map_err(|e| {
            let _ = env.exception_clear();
            e
        })?;
    if listener.is_null() {
        return Ok(false);
    }

    // ClientCommonPacketListenerImpl.connection field (type Connection)
    let connection = env
        .get_field(
            &listener,
            "connection",
            "Lnet/minecraft/network/Connection;",
        )
        .and_then(|v| v.l())
        .map_err(|e| {
            let _ = env.exception_clear();
            e
        })?;
    if connection.is_null() {
        return Ok(false);
    }

    // Connection.channel field (type io.netty.channel.Channel)
    let channel = env
        .get_field(&connection, "channel", "Lio/netty/channel/Channel;")
        .and_then(|v| v.l())
        .map_err(|e| {
            let _ = env.exception_clear();
            e
        })?;
    if channel.is_null() {
        return Ok(false);
    }

    // Get MC's ClassLoader from the Connection class object
    let conn_class = env.get_object_class(&connection)?;
    let classloader = env
        .call_method(&conn_class, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?
        .l()?;
    if classloader.is_null() {
        anyhow::bail!("Connection ClassLoader is null (bootstrap?)");
    }

    let interceptor_cls = define_class(env, &classloader)?;

    env.register_native_methods(
        &interceptor_cls,
        &[
            NativeMethod {
                name: "onIncoming".into(),
                sig: "(Ljava/lang/Object;)V".into(),
                fn_ptr: on_incoming_native as *mut std::ffi::c_void,
            },
            NativeMethod {
                name: "onOutgoing".into(),
                sig: "(Ljava/lang/Object;)Z".into(),
                fn_ptr: on_outgoing_native as *mut std::ffi::c_void,
            },
        ],
    )?;

    let instance = env.new_object(&interceptor_cls, "()V", &[])?;
    let pipeline = env
        .call_method(&channel, "pipeline", "()Lio/netty/channel/ChannelPipeline;", &[])?
        .l()?;
    if pipeline.is_null() {
        anyhow::bail!("pipeline() returned null");
    }

    let handler_name = env.new_string("anemoia_interceptor")?;
    let handler_name_obj: JObject = handler_name.into();

    // Try to insert after "decoder" (post-decode position, sees MC packet objects).
    // Falls back to addLast if "decoder" handler not present.
    let after_name = env.new_string("decoder")?;
    let after_name_obj: JObject = after_name.into();
    let added_after = env.call_method(
        &pipeline,
        "addAfter",
        "(Ljava/lang/String;Ljava/lang/String;Lio/netty/channel/ChannelHandler;)Lio/netty/channel/ChannelPipeline;",
        &[
            JValue::Object(&after_name_obj),
            JValue::Object(&handler_name_obj),
            JValue::Object(&instance),
        ],
    );
    if added_after.is_err() {
        let _ = env.exception_clear();
        // decoder not found — fall back to addLast (still after raw-byte handlers)
        let handler_name2 = env.new_string("anemoia_interceptor")?;
        let handler_name2_obj: JObject = handler_name2.into();
        env.call_method(
            &pipeline,
            "addLast",
            "(Ljava/lang/String;Lio/netty/channel/ChannelHandler;)Lio/netty/channel/ChannelPipeline;",
            &[JValue::Object(&handler_name2_obj), JValue::Object(&instance)],
        )?;
    }

    Ok(true)
}

#[cfg(incoming_capture)]
fn define_class<'local>(
    env: &mut JNIEnv<'local>,
    classloader: &JObject<'local>,
) -> Result<jni::objects::JClass<'local>> {
    let bytes = INTERCEPTOR_BYTES;
    let len = bytes.len() as i32;

    let byte_arr = env.byte_array_from_slice(bytes)?;
    let name_str = env.new_string("AnemoiaInterceptor")?;

    // Convert to JObject so we can pass as JValue::Object
    let byte_arr_obj: JObject = byte_arr.into();
    let name_obj: JObject = name_str.into();

    // JNI bypasses Java access control — call protected ClassLoader.defineClass directly
    let cls_obj = env
        .call_method(
            classloader,
            "defineClass",
            "(Ljava/lang/String;[BII)Ljava/lang/Class;",
            &[
                JValue::Object(&name_obj),
                JValue::Object(&byte_arr_obj),
                JValue::Int(0),
                JValue::Int(len),
            ],
        )
        .map_err(|e| {
            let _ = env.exception_clear();
            e
        })?
        .l()?;

    if cls_obj.is_null() {
        anyhow::bail!("defineClass returned null");
    }
    Ok(jni::objects::JClass::from(cls_obj))
}

unsafe extern "C" fn on_incoming_native(
    raw_env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
    packet: jni::sys::jobject,
) {
    let mut env = match JNIEnv::from_raw(raw_env) {
        Ok(e) => e,
        Err(_) => return,
    };
    let packet_obj = JObject::from_raw(packet);

    let cls = match env.get_object_class(&packet_obj) {
        Ok(c) => c,
        Err(_) => return,
    };
    let name_jstr = match env
        .call_method(&cls, "getName", "()Ljava/lang/String;", &[])
        .and_then(|v| v.l())
    {
        Ok(o) if !o.is_null() => o,
        _ => return,
    };
    let type_name: String = match env.get_string((&name_jstr).into()) {
        Ok(s) => s.into(),
        Err(_) => return,
    };

    let global_ref = match env.new_global_ref(&packet_obj) {
        Ok(r) => r,
        Err(_) => return,
    };

    crate::packet_capture::push_in(type_name, global_ref);
}

#[cfg(incoming_capture)]
unsafe extern "C" fn on_outgoing_native(
    raw_env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
    packet: jni::sys::jobject,
) -> jni::sys::jboolean {
    let mut env = match JNIEnv::from_raw(raw_env) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    let packet_obj = JObject::from_raw(packet);

    // Get type name
    let cls = match env.get_object_class(&packet_obj) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    let name_jstr = match env
        .call_method(&cls, "getName", "()Ljava/lang/String;", &[])
        .and_then(|v| v.l())
    {
        Ok(o) if !o.is_null() => o,
        _ => return 0,
    };
    let type_name: String = match env.get_string((&name_jstr).into()) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };

    // Check if this packet was sent by Lua (bypass flag)
    if crate::mc::packet::is_lua_send(&mut env, &packet_obj) {
        if let Ok(global) = env.new_global_ref(&packet_obj) {
            crate::packet_capture::push_out(type_name, global, false);
        }
        return 0;
    }

    // Hard-cancel movement packets when FreeCam freeze is active.
    // This runs without acquiring the Lua lock, closing the race window where
    // on_tick holds the lock and movement packets slip through unintercepted.
    if FREEZE_PACKETS.load(Ordering::Acquire) && type_name.contains("ServerboundMovePlayerPacket") {
        if let Ok(cap_ref) = env.new_global_ref(&packet_obj) {
            crate::packet_capture::push_out(type_name, cap_ref, true);
        }
        return 1;
    }

    let global_ref = match env.new_global_ref(&packet_obj) {
        Ok(r) => r,
        Err(_) => return 0,
    };

    // Run Lua on_packet_send callbacks (non-blocking: if Lua is busy, let packet through)
    let cancelled = crate::lua_engine::try_on_packet_send(global_ref).unwrap_or(false);

    if let Ok(cap_ref) = env.new_global_ref(&packet_obj) {
        crate::packet_capture::push_out(type_name, cap_ref, cancelled);
    }

    cancelled as jni::sys::jboolean
}
