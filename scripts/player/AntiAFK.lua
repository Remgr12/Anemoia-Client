local module = {
    name = "AntiAFK",
    description = "Prevents you from being kicked for AFK",
    category = "Player",
    enabled = false,
    settings = {
        delay = 1.0, -- seconds
    },
    _settings_meta = {
        delay = { min = 0.1, max = 5.0 }
    },
    last_move = 0
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local now = os.clock()
    if now - self.last_move < self.settings.delay then return end

    local ok, yaw = pcall(function() return player:yaw() end)
    if not ok then return end
    pcall(function() player:set_yaw(yaw + 1) end)
    self.last_move = now
end

anemoia.register(module)
