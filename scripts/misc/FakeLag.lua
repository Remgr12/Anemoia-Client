local module = {
    name = "FakeLag",
    description = "Buffers and sends packets in bursts",
    category = "Misc",
    enabled = false,
    settings = {
        delay = 0.2, -- seconds
    },
    _settings_meta = {
        delay = { min = 0.05, max = 2.0 }
    },
    packets = {},
    last_send = 0
}

anemoia.on_packet_send(function(packet)
    if not module.enabled then return false end
    
    -- Buffer the packet and cancel the original send
    table.insert(module.packets, packet)
    return true
end)

function module:on_tick()
    local now = os.clock()
    if now - self.last_send > self.settings.delay then
        -- Send all buffered packets raw (bypassing our own hook)
        for _, p in ipairs(self.packets) do
            mc.send_packet(p, true)
        end
        self.packets = {}
        self.last_send = now
    end
end

function module:on_disable()
    -- Send remaining packets when disabling
    for _, p in ipairs(self.packets) do
        mc.send_packet(p, true)
    end
    self.packets = {}
end

anemoia.register(module)
