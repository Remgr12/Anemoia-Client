local module = {
    name = "Criticals",
    description = "Forces critical hits",
    category = "Combat",
    enabled = false,
    settings = {
        mode = "Packet", -- Packet, Jump
    },
    _settings_meta = {
        mode = { type = "enum", options = { "Packet", "Jump" } }
    }
}

anemoia.on_packet_send(function(packet)
    if not module.enabled then return end
    
    -- We can't easily check packet type by name yet in a fast way,
    -- but usually attack packets are what we want to hook.
    -- For now, let's use a simpler on_tick approach for Criticals
    -- that spoofs position when attacking.
end)

function module:on_tick()
    local player = mc.player()
    if not player then return end

    -- Simplified: if we are about to attack (left click) and on ground
    if mc.is_key_down(0) and player:on_ground() then
        local mode = self.settings.mode
        local x, y, z = player:x(), player:y(), player:z()
        
        if mode == "Packet" then
            -- Sending small movements to trick server into thinking we are in air
            local p1 = anemoia.create_position_packet(x, y + 0.0625, z, false)
            local p2 = anemoia.create_position_packet(x, y, z, false)
            mc.send_packet(p1)
            mc.send_packet(p2)
        elseif mode == "Jump" then
            player:jump()
        end
    end
end

anemoia.register(module)
