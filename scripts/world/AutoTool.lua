local module = {
    name = "AutoTool",
    description = "Automatically selects the best tool for the targeted block",
    category = "World",
    enabled = false,
    _saved_slot = nil,
}

-- Priority score by item type_id substring. Higher = better for mining.
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
        if player then player:inventory():set_selected_slot(self._saved_slot) end
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
            player:inventory():set_selected_slot(self._saved_slot)
            self._saved_slot = nil
        end
        return
    end

    local inv = player:inventory()
    local orig_slot = inv:selected_slot()

    local best_slot  = orig_slot
    local best_score = -1

    for slot = 0, 8 do
        inv:set_selected_slot(slot)
        local item = player:main_hand_item()
        if not item:is_empty() then
            local s = tool_score(item:type_id())
            if s > best_score then
                best_score = s
                best_slot  = slot
            end
        end
    end

    if not self._saved_slot then
        self._saved_slot = orig_slot
    end

    inv:set_selected_slot(best_slot)
end

anemoia.register(module)
