local module = {
    name = "Velocity",
    description = "Reduces or removes knockback",
    category = "Combat",
    enabled = false,
    settings = {
        horizontal = 0, -- %
        vertical = 0,   -- %
        mode = "Simple", -- Simple, JumpReset
    },
    _settings_meta = {
        horizontal = { min = 0, max = 100 },
        vertical = { min = 0, max = 100 },
        mode = { type = "enum", options = { "Simple", "JumpReset" } }
    },
    was_hurt = false
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local ok_ht, hurt_time = pcall(function() return player:hurt_time() end)
    if not ok_ht then return end

    local mode = self.settings.mode

    if mode == "Simple" then
        if hurt_time > 0 and not self.was_hurt then
            local ok_vel, vel = pcall(function() return player:velocity() end)
            if ok_vel then
                local vx, vy, vz = table.unpack(vel)
                pcall(function()
                    player:set_velocity(
                        vx * (self.settings.horizontal / 100),
                        vy * (self.settings.vertical / 100),
                        vz * (self.settings.horizontal / 100)
                    )
                end)
            end
            self.was_hurt = true
        elseif hurt_time == 0 then
            self.was_hurt = false
        end
    elseif mode == "JumpReset" then
        if hurt_time > 0 and not self.was_hurt then
            local ok_g, on_ground = pcall(function() return player:on_ground() end)
            if ok_g and on_ground then
                local ok_vel, vel = pcall(function() return player:velocity() end)
                if ok_vel then
                    local vx, _, vz = table.unpack(vel)
                    pcall(function() player:set_velocity(vx, 0.42, vz) end)
                end
            end
            self.was_hurt = true
        elseif hurt_time == 0 then
            self.was_hurt = false
        end
    end
end

anemoia.register(module)
