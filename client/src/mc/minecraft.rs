use anyhow::Result;
use jni::{objects::GlobalRef, JNIEnv};

use super::paths;

pub struct Minecraft {
    pub jni_ref: GlobalRef,
}

impl Minecraft {
    /// Calls `net.minecraft.client.Minecraft.getInstance()` and returns a handle.
    ///
    /// Returns `None` when called before the game has finished initialising its
    /// singleton (e.g. during loading screen).
    pub fn get_instance(env: &mut JNIEnv) -> Result<Option<Self>> {
        let cls = crate::jvm::Jvm::find_class(env, paths::MINECRAFT)?;
        let obj = env
            .call_static_method(
                &cls,
                "getInstance",
                concat!("()Lnet/minecraft/client/Minecraft;"),
                &[],
            )?
            .l()?;

        if obj.is_null() {
            return Ok(None);
        }

        let global = env.new_global_ref(obj)?;
        Ok(Some(Minecraft { jni_ref: global }))
    }

    pub fn left_click_mouse(&self, env: &mut JNIEnv) -> Result<()> {
        env.call_method(self.jni_ref.as_obj(), "leftClickMouse", "()V", &[])?;
        Ok(())
    }

    pub fn right_click_mouse(&self, env: &mut JNIEnv) -> Result<()> {
        env.call_method(self.jni_ref.as_obj(), "rightClickMouse", "()V", &[])?;
        Ok(())
    }

    pub fn use_item(&self, env: &mut JNIEnv, hand: &str) -> Result<()> {
        let gamemode = env.get_field(self.jni_ref.as_obj(), "gameMode", "Lnet/minecraft/client/multiplayer/MultiPlayerGameMode;")?.l()?;
        let player = env.get_field(self.jni_ref.as_obj(), "player", "Lnet/minecraft/client/player/LocalPlayer;")?.l()?;
        
        let hand_cls = crate::jvm::Jvm::find_class(env, "net/minecraft/world/InteractionHand")?;
        let hand_obj = env.get_static_field(hand_cls, hand, "Lnet/minecraft/world/InteractionHand;")?.l()?;

        env.call_method(&gamemode, "useItem", "(Lnet/minecraft/world/entity/player/Player;Lnet/minecraft/world/InteractionHand;)Lnet/minecraft/world/InteractionResult;", &[
            jni::objects::JValue::Object(&player),
            jni::objects::JValue::Object(&hand_obj),
        ])?;
        Ok(())
    }

    pub fn get_hit_result(&self, env: &mut JNIEnv) -> Result<Option<super::world::HitResult>> {
        let obj = env.get_field(self.jni_ref.as_obj(), "hitResult", "Lnet/minecraft/world/phys/HitResult;")?.l()?;
        if obj.is_null() {
            return Ok(None);
        }
        Ok(Some(super::world::HitResult { jni_ref: env.new_global_ref(obj)? }))
    }

    pub fn get_current_screen(&self, env: &mut JNIEnv) -> Result<Option<GlobalRef>> {
        let obj = env.get_field(self.jni_ref.as_obj(), "screen", "Lnet/minecraft/client/gui/screens/Screen;")?.l()?;
        if obj.is_null() {
            return Ok(None);
        }
        Ok(Some(env.new_global_ref(obj)?))
    }

    pub fn get_right_click_delay_timer(&self, env: &mut JNIEnv) -> Result<i32> {
        Ok(env.get_field(self.jni_ref.as_obj(), "rightClickDelayTimer", "I")?.i()?)
    }

    pub fn set_right_click_delay_timer(&self, env: &mut JNIEnv, value: i32) -> Result<()> {
        env.set_field(self.jni_ref.as_obj(), "rightClickDelayTimer", "I", jni::objects::JValue::Int(value))?;
        Ok(())
    }

    pub fn set_gamma(&self, env: &mut JNIEnv, value: f64) -> Result<()> {
        let options = env.get_field(self.jni_ref.as_obj(), "options", "Lnet/minecraft/client/Options;")?.l()?;
        let gamma_option = env.get_field(&options, "gamma", "Lnet/minecraft/client/OptionInstance;")?.l()?;
        
        let double_val = env.new_object("java/lang/Double", "(D)V", &[jni::objects::JValue::Double(value)])?;
        env.call_method(&gamma_option, "set", "(Ljava/lang/Object;)V", &[
            jni::objects::JValue::Object(&double_val)
        ])?;
        Ok(())
    }

    pub fn inventory_click(&self, env: &mut JNIEnv, container_id: i32, slot: i32, _button: i32, click_type: &str) -> Result<()> {
        let gamemode = env.get_field(self.jni_ref.as_obj(), "gameMode", "Lnet/minecraft/client/multiplayer/MultiPlayerGameMode;")?.l()?;
        let click_type_cls = crate::jvm::Jvm::find_class(env, "net/minecraft/world/inventory/ClickType")?;
        let click_type_obj = env.get_static_field(click_type_cls, click_type, "Lnet/minecraft/world/inventory/ClickType;")?.l()?;
        
        let player = env.get_field(self.jni_ref.as_obj(), "player", "Lnet/minecraft/client/player/LocalPlayer;")?.l()?;

        env.call_method(&gamemode, "handleInventoryMouseClick", "(IILnet/minecraft/world/inventory/ClickType;Lnet/minecraft/world/entity/player/Player;)V", &[
            jni::objects::JValue::Int(container_id),
            jni::objects::JValue::Int(slot),
            jni::objects::JValue::Object(&click_type_obj),
            jni::objects::JValue::Object(&player),
        ])?;
        Ok(())
    }
}
