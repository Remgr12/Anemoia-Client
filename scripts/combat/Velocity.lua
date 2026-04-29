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

    local hurt_time = player:hurt_time()
    local mode = self.settings.mode

    if mode == "Simple" then
        if hurt_time > 0 and not self.was_hurt then
            local vx, vy, vz = table.unpack(player:velocity())
            player:set_velocity(
                vx * (self.settings.horizontal / 100),
                vy * (self.settings.vertical / 100),
                vz * (self.settings.horizontal / 100)
            )
            self.was_hurt = true
        elseif hurt_time == 0 then
            self.was_hurt = false
        end
    elseif mode == "JumpReset" then
        if hurt_time > 0 and not self.was_hurt then
            if player:on_ground() then
                -- Jump reset logic: jumping when hit reduces knockback
                local input = player:input()
                -- We can't easily force a jump via input yet, 
                -- but we can set Y velocity
                local vx, vy, vz = table.unpack(player:velocity())
                player:set_velocity(vx, 0.42, vz)
            end
            self.was_hurt = true
        elseif hurt_time == 0 then
            self.was_hurt = false
        end
    end
end

anemoia.register(module)
