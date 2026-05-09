local module = {
    name = "HighJump",
    description = "Increases your jump height",
    category = "Movement",
    enabled = false,
    settings = {
        velocity = 0.7,
    },
    _settings_meta = {
        velocity = { min = 0.42, max = 2.0 }
    },
    _boosted = false,
}

-- Boost once per jump: apply on the first airborne tick where vy > 0.
-- Checking vy ≈ 0.42 at the exact jump tick is unreliable because MC applies
-- gravity before our hook fires, so by the time on_ground flips to false vy
-- has already dropped to ~0.34.  Using a _boosted flag avoids the race.
function module:on_tick()
    local player = mc.player()
    if not player then return end

    local ok_g, on_ground = pcall(function() return player:on_ground() end)
    if not ok_g then return end

    if on_ground then
        self._boosted = false
        return
    end

    if self._boosted then return end

    local ok_vel, vel = pcall(function() return player:velocity() end)
    if not ok_vel then return end
    local vx, vy, vz = table.unpack(vel)

    if vy > 0 then
        pcall(function() player:set_velocity(vx, self.settings.velocity, vz) end)
        self._boosted = true
    end
end

anemoia.register(module)
