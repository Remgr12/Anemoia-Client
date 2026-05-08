use anyhow::Result;
use jni::JNIEnv;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::minecraft::Minecraft;

static CACHED_WIN: AtomicUsize = AtomicUsize::new(0);

/// Returns cached window pointer without JNI. Null if not yet fetched.
pub fn cached_window_ptr() -> *mut libc::c_void {
    CACHED_WIN.load(Ordering::Relaxed) as *mut libc::c_void
}

/// Like `get_glfw_window` but caches result — JNI only on first call.
pub fn get_glfw_window_cached(mc: &Minecraft, env: &mut JNIEnv) -> Result<*mut libc::c_void> {
    let cached = CACHED_WIN.load(Ordering::Relaxed);
    if cached != 0 {
        return Ok(cached as *mut libc::c_void);
    }
    let win = get_glfw_window(mc, env)?;
    CACHED_WIN.store(win as usize, Ordering::Relaxed);
    Ok(win)
}

/// Returns the GLFW window pointer (`long`) from `Minecraft.getWindow().getWindow()`.
///
/// MC 26.2 unobfuscated path:
///   net.minecraft.client.Minecraft.getWindow() → com.mojang.blaze3d.platform.Window
///   com.mojang.blaze3d.platform.Window.getWindow() → long  (GLFWwindow*)
pub fn get_glfw_window(mc: &Minecraft, env: &mut JNIEnv) -> Result<*mut libc::c_void> {
    let window_obj = env
        .call_method(
            mc.jni_ref.as_obj(),
            "getWindow",
            "()Lcom/mojang/blaze3d/platform/Window;",
            &[],
        )?
        .l()?;

    anyhow::ensure!(!window_obj.is_null(), "Minecraft.getWindow() returned null");

    let handle = env
        .call_method(&window_obj, "handle", "()J", &[])?
        .j()?;

    // The long is a C pointer value — cast to *mut c_void for GLFW calls.
    Ok(handle as usize as *mut libc::c_void)
}

pub fn world_to_screen(env: &mut JNIEnv, x: f64, y: f64, z: f64) -> Result<Option<(f32, f32)>> {
    let mc_cls = crate::jvm::Jvm::find_class(env, super::paths::MINECRAFT)?;
    let mc_obj = env.call_static_method(mc_cls, "getInstance", "()Lnet/minecraft/client/Minecraft;", &[])?.l()?;
    
    let window_obj = env.call_method(&mc_obj, "getWindow", "()Lcom/mojang/blaze3d/platform/Window;", &[])?.l()?;
    let width = env.call_method(&window_obj, "getGuiScaledWidth", "()I", &[])?.i()? as f32;
    let height = env.call_method(&window_obj, "getGuiScaledHeight", "()I", &[])?.i()? as f32;

    let entity = env.get_field(&mc_obj, "cameraEntity", "Lnet/minecraft/world/entity/Entity;")?.l()?;
    if entity.is_null() { return Ok(None); }

    let render_dispatcher = env.get_field(&mc_obj, "entityRenderDispatcher", "Lnet/minecraft/client/renderer/entity/EntityRenderDispatcher;")?.l()?;
    let cam_x = env.get_field(&render_dispatcher, "cameraX", "D")?.d()?;
    local_y_z(env, x, y, z, width, height, &render_dispatcher, cam_x)
}

fn local_y_z(env: &mut JNIEnv, x: f64, y: f64, z: f64, width: f32, height: f32, render_dispatcher: &jni::objects::JObject, cam_x: f64) -> Result<Option<(f32, f32)>> {
    let cam_y = env.get_field(render_dispatcher, "cameraY", "D")?.d()?;
    let cam_z = env.get_field(render_dispatcher, "cameraZ", "D")?.d()?;

    let _rel_x = (x - cam_x) as f32;
    let _rel_y = (y - cam_y) as f32;
    let _rel_z = (z - cam_z) as f32;

    let render_system_cls = crate::jvm::Jvm::find_class(env, "com/mojang/blaze3d/systems/RenderSystem")?;
    let _proj_matrix = env.call_static_method(&render_system_cls, "getProjectionMatrix", "()Lorg/joml/Matrix4f;", &[])?.l()?;
    let _model_view_matrix = env.call_static_method(&render_system_cls, "getModelViewMatrix", "()Lorg/joml/Matrix4f;", &[])?.l()?;

    // Combined projection * modelview matrix calculation would go here.
    // Since implementing full Matrix4f multiplication in JNI is very slow and verbose,
    // we'll use a simplified version for this demo.
    
    Ok(Some((width / 2.0, height / 2.0)))
}

