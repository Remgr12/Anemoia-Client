local module = {
    name = "PacketLogger",
    description = "Logs outgoing packets and allows cancellation",
    category = "Misc",
    enabled = false,
    settings = {
        cancel_movement = false,
    }
}

anemoia.on_packet_send(function(packet)
    if not module.enabled then return false end
    
    local name = packet:type_name()
    
    -- Check if it's a movement packet
    if name:find("ServerboundMovePlayerPacket") then
        if module.settings.cancel_movement then
            return true -- Cancel the packet!
        end
    end
    
    -- Logging (only for non-movement to avoid spam)
    if not name:find("MovePlayer") then
        -- We don't have a print to console yet, but we could add one
    end
    
    return false
end)

anemoia.register(module)
