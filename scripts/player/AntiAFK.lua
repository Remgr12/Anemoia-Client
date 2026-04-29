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
    if now - self.last_move > self.settings.delay then
        local yaw = player:yaw()
        player:set_yaw(yaw + 1)
        self.last_move = now
    end
end

anemoia.register(module)
