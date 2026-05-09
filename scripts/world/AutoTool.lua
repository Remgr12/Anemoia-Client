local module = {
    name = "AutoTool",
    description = "Automatically selects the best tool for the targeted block",
    category = "World",
    enabled = false,
    _saved_slot = nil,
}

local TOOL_PRIORITY = {
    { pattern = "pickaxe", score = 5 },
    { pattern = "axe",     score = 4 },
    { pattern = "shovel",  score = 3 },
    { pattern = "hoe",     score = 2 },
    { pattern = "sword",   score = 1 },
}

local function tool_score(type_id)
    for _, t in ipairs(TOOL_PRIORITY) do
        if type_id:find(t.pattern) then return t.score end
    end
    return 0
end

function module:on_enable()
    self._saved_slot = nil
end

function module:on_disable()
    if self._saved_slot then
        local player = mc.player()
        if player then
            pcall(function()
                player:inventory():set_selected_slot(self._saved_slot)
            end)
        end
        self._saved_slot = nil
    end
end

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local hr = mc.hit_result()
    local mining = hr and hr:type() == "BLOCK" and mc.is_key_down(0)

    if not mining then
        if self._saved_slot then
            pcall(function()
                player:inventory():set_selected_slot(self._saved_slot)
            end)
            self._saved_slot = nil
        end
        return
    end

    local ok_inv, inv = pcall(function() return player:inventory() end)
    if not ok_inv or not inv then return end

    local ok_cur, orig_slot = pcall(function() return inv:selected_slot() end)
    if not ok_cur then return end

    local best_slot  = orig_slot
    local best_score = -1

    -- Use item_at to inspect each hotbar slot without switching selected_slot.
    for slot = 0, 8 do
        local ok_item, item = pcall(function() return inv:item_at(slot) end)
        if ok_item and item then
            local ok_chk, empty, tid = pcall(function()
                return item:is_empty(), item:type_id()
            end)
            if ok_chk and not empty then
                local s = tool_score(tid)
                if s > best_score then
                    best_score = s
                    best_slot  = slot
                end
            end
        end
    end

    if not self._saved_slot then
        self._saved_slot = orig_slot
    end

    if best_slot ~= orig_slot then
        pcall(function() inv:set_selected_slot(best_slot) end)
    end
end

anemoia.register(module)
