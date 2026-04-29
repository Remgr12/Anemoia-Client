local module = {
    name = "ChestStealer",
    description = "Automatically steals items from chests",
    category = "Player",
    enabled = false,
    settings = {
        delay = 0.1,
        close_when_done = true,
    },
    _settings_meta = {
        delay = { min = 0.0, max = 1.0 }
    },
    last_steal = 0
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local container_id = player:container_id()
    if container_id == 0 then return end -- 0 is usually the player inventory

    local now = os.clock()
    if now - self.last_steal < self.settings.delay then
        return
    end

    -- In a real implementation, we would check if slots have items.
    -- For this port, we will just try to 'Quick Move' slots 0 to 26 (typical chest)
    -- This is inefficient but demonstrates the API.
    
    -- We need a way to check if a slot is empty to know when we are done.
    -- Since we don't have that yet, let's just do one slot per delay.
    
    local start_slot = 0
    local end_slot = 26
    
    -- We'll just pick a random slot to steal for this demo
    local slot = math.random(start_slot, end_slot)
    mc.inventory_click(container_id, slot, 0, "QUICK_MOVE")
    
    self.last_steal = now
end

anemoia.register(module)
