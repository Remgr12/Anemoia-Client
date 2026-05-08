local module = {
    name = "Criticals",
    description = "Forces critical hits by timing position packets or jumps",
    category = "Combat",
    enabled = false,
    settings = {
        mode = "Packet",
    },
    _settings_meta = {
        mode = { type = "enum", options = { "Packet", "Jump" } }
    },
    _last_crit = 0,
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    if not mc.is_key_down(0) then return end

    local now = os.clock() * 1000
    if now - self._last_crit < 100 then return end

    local mode = self.settings.mode

    if mode == "Packet" then
        local ok_pos, x, y, z = pcall(function() return player:x(), player:y(), player:z() end)
        if not ok_pos then return end
        local ok_g, on_ground = pcall(function() return player:on_ground() end)
        if not ok_g or not on_ground then return end
        local p1 = anemoia.create_position_packet(x, y + 0.0625, z, false)
        local p2 = anemoia.create_position_packet(x, y, z, false)
        pcall(function() mc.send_packet(p1) end)
        pcall(function() mc.send_packet(p2) end)
        self._last_crit = now

    elseif mode == "Jump" then
        local ok_g, on_ground = pcall(function() return player:on_ground() end)
        if not ok_g or not on_ground then return end
        pcall(function() player:jump() end)
        self._last_crit = now
    end
end

anemoia.register(module)
