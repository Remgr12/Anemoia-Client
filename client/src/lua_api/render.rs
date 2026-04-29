use mlua::prelude::*;
use egui::{Painter, Pos2, Color32, Stroke, Rect, Rounding};

pub struct LuaPainter {
    pub painter: Painter,
}

impl LuaUserData for LuaPainter {
    fn add_methods<M: LuaUserDataMethods<Self>>(m: &mut M) {
        m.add_method("line", |_, this, (x1, y1, x2, y2, r, g, b, a, width): (f32, f32, f32, f32, u8, u8, u8, u8, f32)| {
            this.painter.line_segment(
                [Pos2::new(x1, y1), Pos2::new(x2, y2)],
                Stroke::new(width, Color32::from_rgba_unmultiplied(r, g, b, a))
            );
            Ok(())
        });

        m.add_method("rect", |_, this, (x, y, w, h, r, g, b, a, rounding): (f32, f32, f32, f32, u8, u8, u8, u8, f32)| {
            this.painter.rect_filled(
                Rect::from_min_size(Pos2::new(x, y), egui::vec2(w, h)),
                Rounding::same(rounding),
                Color32::from_rgba_unmultiplied(r, g, b, a)
            );
            Ok(())
        });

        m.add_method("rect_outline", |_, this, (x, y, w, h, r, g, b, a, width, rounding): (f32, f32, f32, f32, u8, u8, u8, u8, f32, f32)| {
            this.painter.rect_stroke(
                Rect::from_min_size(Pos2::new(x, y), egui::vec2(w, h)),
                Rounding::same(rounding),
                Stroke::new(width, Color32::from_rgba_unmultiplied(r, g, b, a))
            );
            Ok(())
        });

        m.add_method("text", |_, this, (x, y, text, r, g, b, a, size): (f32, f32, String, u8, u8, u8, u8, f32)| {
            this.painter.text(
                Pos2::new(x, y),
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::proportional(size),
                Color32::from_rgba_unmultiplied(r, g, b, a)
            );
            Ok(())
        });
    }
}

pub fn register(lua: &Lua, anemoia: &LuaTable) -> anyhow::Result<()> {
    anemoia.set("on_render", lua.create_function(|lua, func: LuaFunction| {
        let registry_key = lua.create_registry_value(func)?;
        let callbacks = crate::lua_engine::get_render_callbacks();
        callbacks.lock().push(registry_key);
        Ok(())
    })?)?;

    anemoia.set("on_zulip_ui", lua.create_function(|lua, func: LuaFunction| {
        let registry_key = lua.create_registry_value(func)?;
        let cb = crate::lua_engine::get_zulip_ui_callback();
        *cb.lock() = Some(registry_key);
        Ok(())
    })?)?;

    Ok(())
}
