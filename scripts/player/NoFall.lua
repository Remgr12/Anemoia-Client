local module = {
    name = "NoFall",
    description = "Prevents fall damage",
    category = "Player",
    enabled = false,
    settings = {
        mode = "Spoof", -- Spoof, Packet
    },
    _settings_meta = {
        mode = { type = "enum", options = { "Spoof", "Packet" } }
    }
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local fall_dist = player:fall_distance()
    local mode = self.settings.mode

    if mode == "Spoof" then
        if fall_dist > 2.0 then
            player:set_on_ground(true)
        end
    elseif mode == "Packet" then
        if fall_dist > 2.0 then
            local x, y, z = player:x(), player:y(), player:z()
            local packet = anemoia.create_position_packet(x, y, z, true)
            mc.send_packet(packet)
            -- We don't have a way to set fall distance directly yet,
            -- but sending the packet should tell the server we landed.
        end
    end
end

anemoia.register(module)
