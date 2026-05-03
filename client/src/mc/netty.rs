use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

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
        &[NativeMethod {
            name: "onIncoming".into(),
            sig: "(Ljava/lang/Object;)V".into(),
            fn_ptr: on_incoming_native as *mut std::ffi::c_void,
        }],
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
    env.call_method(
        &pipeline,
        "addFirst",
        "(Ljava/lang/String;Lio/netty/channel/ChannelHandler;)Lio/netty/channel/ChannelPipeline;",
        &[JValue::Object(&handler_name_obj), JValue::Object(&instance)],
    )?;

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
