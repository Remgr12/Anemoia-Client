local module = {
    name = "PacketLogger",
    description = "Logs outgoing packets to chat HUD and allows cancellation",
    category = "Misc",
    enabled = false,
    hidden = true,
    settings = {
        cancel_movement = false,
        log_packets     = true,
    },
    _queue = {},
}

function module:on_tick()
    if not self.enabled or not self.settings.log_packets then
        self._queue = {}
        return
    end
    if #self._queue == 0 then return end

    -- Drain one queued name per tick so the chat HUD isn't flooded.
    local player = mc.player()
    if not player then return end
    pcall(function() player:display_message("§8[Pkt] §7" .. table.remove(self._queue, 1)) end)
end

anemoia.on_packet_send(function(packet)
    if not module.enabled then return false end

    local name = packet:type_name()
    local is_movement = name:find("MovePlayer") ~= nil

    if is_movement and module.settings.cancel_movement then
        return true
    end

    if module.settings.log_packets and not is_movement then
        local short = name:match("([^%.]+)$") or name
        if #module._queue < 20 then
            table.insert(module._queue, short)
        end
    end

    return false
end)

anemoia.register(module)
