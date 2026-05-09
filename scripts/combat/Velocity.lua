local module = {
    name = "Velocity",
    description = "Reduces or removes knockback",
    category = "Combat",
    enabled = false,
    settings = {
        horizontal  = 0,        -- % of knockback to keep (0 = full cancel)
        vertical    = 0,        -- % of vertical knockback to keep
        mode        = "Simple",
        tick_delay  = 0,        -- game ticks (~50ms each) to wait before applying
        chance      = 100,      -- % activation chance (JumpReset legitimacy)
    },
    _settings_meta = {
        horizontal  = { min = 0, max = 100 },
        vertical    = { min = 0, max = 100 },
        tick_delay  = { min = 0, max = 3 },
        chance      = { min = 0, max = 100 },
        mode        = { type = "enum", options = { "Simple", "JumpReset" } }
    },
    _was_hurt  = false,
    _hit_time  = 0,
    _pending   = false,
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local ok_ht, hurt_time = pcall(function() return player:hurt_time() end)
    if not ok_ht then return end

    local now = os.clock() * 1000

    if hurt_time > 0 and not self._was_hurt then
        self._was_hurt = true
        self._hit_time = now
        self._pending  = true
    elseif hurt_time == 0 then
        self._was_hurt = false
    end

    if not self._pending then return end

    -- Wait tick_delay * 50ms before reducing (scrambles heuristic detectors
    -- that analyse the initial acceleration curve post-hit)
    if now - self._hit_time < self.settings.tick_delay * 50 then return end
    self._pending = false

    local mode = self.settings.mode

    if mode == "Simple" then
        local ok_vel, vel = pcall(function() return player:velocity() end)
        if ok_vel then
            local vx, vy, vz = table.unpack(vel)
            pcall(function()
                player:set_velocity(
                    vx * (self.settings.horizontal / 100),
                    vy * (self.settings.vertical   / 100),
                    vz * (self.settings.horizontal / 100)
                )
            end)
        end

    elseif mode == "JumpReset" then
        if math.random(100) > self.settings.chance then return end
        local ok_g, on_ground = pcall(function() return player:on_ground() end)
        if ok_g and on_ground then
            local ok_vel, vel = pcall(function() return player:velocity() end)
            if ok_vel then
                local vx, _, vz = table.unpack(vel)
                -- Convert horizontal knockback into upward momentum instead
                pcall(function() player:set_velocity(vx, 0.42, vz) end)
            end
        end
    end
end

anemoia.register(module)
