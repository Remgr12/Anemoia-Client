local module = {
    name = "Scaffold",
    description = "Automatically places blocks under your feet",
    category = "World",
    enabled = false,
    settings = {
        mode = "Normal", -- Normal, GodBridge, Telly
        rotations = true,
    },
    _settings_meta = {
        mode = { type = "enum", options = { "Normal", "GodBridge", "Telly" } },
        rotations = { type = "boolean" }
    }
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local x, y, z = player:x(), player:y() - 1, player:z()
    local bx, by, bz = math.floor(x), math.floor(y), math.floor(z)
    
    local block = mc.block(bx, by, bz)
    if block:type_id() == "block.minecraft.air" then
        -- We need a block to place. Check main hand.
        local item = player:main_hand_item()
        if not item:is_empty() and item:type_id():find("block") then
            
            if self.settings.rotations then
                player:set_pitch(80) -- Look down
            end
            
            -- Minecraft.rightClickMouse() is more stable as it uses the same logic as Vanilla
            mc.right_click()
            
            -- Mode specific logic
            local mode = self.settings.mode
            if mode == "GodBridge" then
                -- GodBridge logic
            elseif mode == "Telly" then
                if player:on_ground() then
                    player:jump()
                end
            end
        end
    end
end

anemoia.register(module)
