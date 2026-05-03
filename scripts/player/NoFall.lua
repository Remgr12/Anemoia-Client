local module = {
    name        = "NoFall",
    description = "Prevents fall damage",
    category    = "Player",
    enabled     = false,
    settings = {
        mode      = "Spoof",
        threshold = 2.0,
    },
    _settings_meta = {
        mode      = { type = "enum", options = { "Spoof", "Packet" } },
        threshold = { min = 0.5, max = 5.0 }
    }
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local ok_fall, fall_dist = pcall(function() return player:fall_distance() end)
    if not ok_fall then return end
    if fall_dist <= self.settings.threshold then return end

    local mode = self.settings.mode

    if mode == "Spoof" then
        pcall(function() player:set_on_ground(true) end)

    elseif mode == "Packet" then
        local ok_pos, x, y, z = pcall(function()
            return player:x(), player:y(), player:z()
        end)
        if not ok_pos then return end

        local ok_pkt, packet = pcall(function()
            return anemoia.create_position_packet(x, y, z, true)
        end)
        if not ok_pkt or not packet then return end

        pcall(function() mc.send_packet(packet, false) end)
    end
end

anemoia.register(module)
