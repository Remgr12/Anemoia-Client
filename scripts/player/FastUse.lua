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

    if player:is_using_item() then
        local x, y, z = player:x(), player:y(), player:z()
        for i = 1, self.settings.packets do
            local packet = anemoia.create_position_packet(x, y, z, player:on_ground())
            mc.send_packet(packet)
        end
    end
end

anemoia.register(module)
