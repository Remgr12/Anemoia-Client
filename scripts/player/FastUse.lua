local module = {
    name = "FastUse",
    description = "Uses items faster",
    category = "Player",
    enabled = false,
    settings = {
        packets = 20,
    },
    _settings_meta = {
        packets = { min = 1, max = 50 }
    }
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local ok_use, using = pcall(function() return player:is_using_item() end)
    if not ok_use or not using then return end

    local ok_pos, x, y, z, on_g = pcall(function()
        return player:x(), player:y(), player:z(), player:on_ground()
    end)
    if not ok_pos then return end

    for i = 1, self.settings.packets do
        local ok_pkt, packet = pcall(anemoia.create_position_packet, x, y, z, on_g)
        if ok_pkt and packet then pcall(mc.send_packet, packet) end
    end
end

anemoia.register(module)
