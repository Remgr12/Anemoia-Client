local module = {
    name = "WTap",
    description = "Resets sprint after each attack to maximize outgoing knockback",
    category = "Combat",
    enabled = false,
    settings = {
        delay = 50,   -- ms between sprint-stop and sprint-resume
    },
    _settings_meta = {
        delay = { min = 10, max = 200 },
    },
    _reset_at  = 0,
    _resetting = false,
}

-- Intercept outgoing interact (attack) packets to detect attacks from any module
anemoia.on_packet_send(function(packet)
    if not module.enabled then return false end
    local ok, name = pcall(function() return packet:type_name() end)
    if ok and name and name:find("Interact") then
        module._reset_at  = os.clock() * 1000
        module._resetting = true
    end
    return false  -- never cancel, just observe
end)

function module:on_tick()
    if not self._resetting then return end

    local player = mc.player()
    if not player then return end

    local now = os.clock() * 1000

    if now - self._reset_at < self.settings.delay then
        -- Sprint-stop phase
        pcall(function() player:set_sprinting(false) end)
    else
        -- Resume sprint and end reset cycle
        pcall(function() player:set_sprinting(true) end)
        self._resetting = false
    end
end

function module:on_disable()
    self._resetting = false
    local player = mc.player()
    if player then pcall(function() player:set_sprinting(true) end) end
end

anemoia.register(module)
