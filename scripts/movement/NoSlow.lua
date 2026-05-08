local module = {
    name = "NoSlow",
    description = "Prevents slowing down while using items",
    category = "Movement",
    enabled = false,
    settings = {
        multiplier = 1.0, -- 1.0 = 100% speed
    },
    _settings_meta = {
        multiplier = { min = 0.2, max = 1.0 }
    }
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local ok_use, using = pcall(function() return player:is_using_item() end)
    if not ok_use or not using then return end

    local ok_vel, vel = pcall(function() return player:velocity() end)
    if not ok_vel then return end
    local vx, vy, vz = table.unpack(vel)

    local is_moving = mc.is_key_down(87) or mc.is_key_down(65) or
                      mc.is_key_down(83) or mc.is_key_down(68)
    if is_moving then
        -- MC applies 0.2x slowdown to velocity when using item.
        -- Multiply back to restore intended speed.
        local boost = self.settings.multiplier / 0.2
        pcall(function() player:set_velocity(vx * boost, vy, vz * boost) end)
    end
end

anemoia.register(module)
