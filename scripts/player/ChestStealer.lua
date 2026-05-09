local module = {
    name = "ChestStealer",
    description = "Automatically steals items from chests",
    category = "Player",
    enabled = false,
    settings = {
        delay           = 0.05,
        close_when_done = true,
    },
    _settings_meta = {
        delay = { min = 0.0, max = 1.0 }
    },
    _last_steal   = 0,
    _current_slot = 0,
}

local CHEST_SLOTS = 27  -- single chest; double chest = 54

function module:on_enable()
    self._current_slot = 0
end

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local container_id = player:container_id()
    if container_id == 0 then
        self._current_slot = 0
        return
    end

    local now = os.clock()
    if now - self._last_steal < self.settings.delay then return end

    if self._current_slot >= CHEST_SLOTS then
        self._current_slot = 0
        return
    end

    local ok = pcall(mc.inventory_click, container_id, self._current_slot, 0, "QUICK_MOVE")
    if not ok then return end
    self._current_slot = self._current_slot + 1
    self._last_steal = now
end

anemoia.register(module)
