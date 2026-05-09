local module = {
    name = "AutoArmor",
    description = "Equips the best armor from inventory automatically",
    category = "Combat",
    enabled = false,
    settings = {
        delay = 0.5,
    },
    _settings_meta = {
        delay = { min = 0.1, max = 5.0 },
    },
    _last_check = 0,
}

-- Protection value by material (higher = better)
local MATERIAL_SCORE = {
    { pat = "netherite", score = 40 },
    { pat = "diamond",   score = 30 },
    { pat = "iron",      score = 20 },
    { pat = "golden",    score = 10 },
    { pat = "chainmail", score = 8  },
    { pat = "leather",   score = 5  },
}

-- Armor piece types that fit each slot
local SLOTS = {
    { internal = 39, container = 5,  match = "helmet"     },
    { internal = 38, container = 6,  match = "chestplate" },
    { internal = 37, container = 7,  match = "leggings"   },
    { internal = 36, container = 8,  match = "boots"      },
}

-- PlayerInventory.getItem() uses internal indices:
--   0-8:  hotbar, 9-35: main inventory, 36-39: armor, 40: offhand
-- mc.inventory_click container slot for hotbar[s] = s+36, main inv = s directly

local function mat_score(tid)
    for _, m in ipairs(MATERIAL_SCORE) do
        if tid:find(m.pat) then return m.score end
    end
    return 0
end

function module:on_tick()
    local player = mc.player()
    if not player then return end

    -- Only act when no container is open
    local ok_cid, cid = pcall(function() return player:container_id() end)
    if not ok_cid or cid ~= 0 then return end

    local now = os.clock()
    if now - self._last_check < self.settings.delay then return end
    self._last_check = now

    local inv = player:inventory()

    for _, slot_def in ipairs(SLOTS) do
        -- Score of currently equipped piece
        local cur_score = 0
        local ok_cur, cur_item = pcall(function() return inv:item_at(slot_def.internal) end)
        if ok_cur and cur_item then
            local ok_tid, tid = pcall(function() return cur_item:type_id() end)
            local ok_emp, emp = pcall(function() return cur_item:is_empty() end)
            if ok_tid and ok_emp and not emp then
                cur_score = mat_score(tid)
            end
        end

        -- Search hotbar (internal 0-8 → container 36-44)
        for s = 0, 8 do
            local ok_i, item = pcall(function() return inv:item_at(s) end)
            if ok_i and item then
                local ok_e, empty = pcall(function() return item:is_empty() end)
                local ok_t, tid   = pcall(function() return item:type_id() end)
                if ok_e and ok_t and not empty and tid:find(slot_def.match) then
                    if mat_score(tid) > cur_score then
                        mc.inventory_click(0, s + 36, 0, "QUICK_MOVE")
                        break
                    end
                end
            end
        end

        -- Search main inventory (internal 9-35 = container 9-35)
        for s = 9, 35 do
            local ok_i, item = pcall(function() return inv:item_at(s) end)
            if ok_i and item then
                local ok_e, empty = pcall(function() return item:is_empty() end)
                local ok_t, tid   = pcall(function() return item:type_id() end)
                if ok_e and ok_t and not empty and tid:find(slot_def.match) then
                    if mat_score(tid) > cur_score then
                        mc.inventory_click(0, s, 0, "QUICK_MOVE")
                        break
                    end
                end
            end
        end
    end
end

anemoia.register(module)
