use anyhow::Result;
use jni::JNIEnv;

use super::minecraft::Minecraft;

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
        .call_method(&window_obj, "getWindow", "()J", &[])?
        .j()?;

    // The long is a C pointer value — cast to *mut c_void for GLFW calls.
    Ok(handle as usize as *mut libc::c_void)
}
