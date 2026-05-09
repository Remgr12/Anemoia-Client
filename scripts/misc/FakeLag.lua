local module = {
    name = "FakeLag",
    description = "Buffers packets in bursts to desync server position",
    category = "Misc",
    enabled = false,
    settings = {
        delay       = 0.2,    -- base buffer duration in seconds
        mode        = "Latency",
        max_packets = 30,     -- hard cap to prevent OOM on long holds
    },
    _settings_meta = {
        delay       = { min = 0.05, max = 2.0 },
        mode        = { type = "enum", options = { "Latency", "Dynamic", "Repel" } },
        max_packets = { min = 5, max = 100 },
    },
    packets     = {},
    last_send   = 0,
    _next_delay = 0.2,
    _was_hurt   = false,
}

anemoia.on_packet_send(function(packet)
    if not module.enabled then return false end
    if #module.packets >= module.settings.max_packets then
        -- Safety: let it through rather than building an unbounded buffer
        return false
    end
    table.insert(module.packets, packet)
    return true  -- cancel original send
end)

function module:on_tick()
    local now  = os.clock()
    local mode = self.settings.mode
    local flush = false

    if mode == "Latency" then
        -- Fixed delay burst
        flush = (now - self.last_send > self.settings.delay)

    elseif mode == "Dynamic" then
        -- Randomised delay (50–150% of configured value) prevents heuristic
        -- anti-cheats from fingerprinting a static latency pattern
        flush = (now - self.last_send > self._next_delay)
        if flush then
            self._next_delay = self.settings.delay * (0.5 + math.random())
        end

    elseif mode == "Repel" then
        -- Flush when we take damage — the position desync makes the attacker's
        -- next hit land on a ghost position, disrupting their combo
        local player = mc.player()
        if player then
            local ok, ht = pcall(function() return player:hurt_time() end)
            if ok then
                local just_hit = ht > 0 and not self._was_hurt
                self._was_hurt = ht > 0
                -- Also flush every 3× delay as a safety drain
                if just_hit or (now - self.last_send > self.settings.delay * 3) then
                    flush = true
                end
            end
        end
    end

    if flush then
        for _, p in ipairs(self.packets) do
            mc.send_packet(p, true)
        end
        self.packets  = {}
        self.last_send = now
    end
end

function module:on_disable()
    for _, p in ipairs(self.packets) do
        mc.send_packet(p, true)
    end
    self.packets   = {}
    self._was_hurt = false
end

anemoia.register(module)
