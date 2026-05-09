local module = {
    name        = "Scaffold",
    description = "Places blocks under feet while moving; hotbar switching, safewalk, rotation bypass",
    category    = "World",
    enabled     = false,
    settings = {
        mode      = "Normal",
        rotations = "Legit",
        safewalk  = true,
        delay     = 2,
        hotbar    = true,
    },
    _settings_meta = {
        mode      = { type = "enum", options = { "Normal", "Legit", "GodBridge" } },
        rotations = { type = "enum", options = { "Off", "Legit", "Silent" } },
        safewalk  = { type = "boolean" },
        delay     = { type = "number", min = 1, max = 10 },
        hotbar    = { type = "boolean" },
    },
    _last_t         = 0,
    _slot_cache     = nil,
    _slot_cache_age = 0,
    _frame          = 0,
}

local Y_MIN, Y_MAX = -64, 320

local NEIGHBORS = {
    { dx=0,  dy=-1, dz=0,  face="UP"    },
    { dx=-1, dy=0,  dz=0,  face="EAST"  },
    { dx=1,  dy=0,  dz=0,  face="WEST"  },
    { dx=0,  dy=0,  dz=-1, face="SOUTH" },
    { dx=0,  dy=0,  dz=1,  face="NORTH" },
}

local function safe_block(bx, by, bz)
    if by < Y_MIN or by > Y_MAX then return nil end
    local ok, blk = pcall(mc.block, bx, by, bz)
    if not ok or not blk then return nil end
    local ok2, tid = pcall(function() return blk:type_id() end)
    if not ok2 then return nil end
    return tid
end

local function is_solid(tid)
    return tid ~= nil and tid ~= "block.minecraft.air"
end

local function find_block_slot(inv)
    local ok_sel, cur = pcall(function() return inv:selected_slot() end)
    if not ok_sel then return nil end

    local function is_block_slot(s)
        local ok, item = pcall(function() return inv:item_at(s) end)
        if not ok or not item then return false end
        local ok2, empty, tid = pcall(function() return item:is_empty(), item:type_id() end)
        return ok2 and not empty and tid:find("block") ~= nil
    end

    if is_block_slot(cur) then return cur end
    for s = 0, 8 do
        if s ~= cur and is_block_slot(s) then return s end
    end
    return nil
end

local function apply_safewalk(player, px, py, pz)
    local ok_vel, vel = pcall(function() return player:velocity() end)
    if not ok_vel then return end
    local vx, vy, vz = vel[1], vel[2], vel[3]
    local changed = false
    local floor_y = math.floor(py) - 1

    if math.abs(vx) > 0.001 then
        local nx = math.floor(px + vx + (vx > 0 and 0.31 or -0.31))
        if not is_solid(safe_block(nx, floor_y, math.floor(pz))) then
            vx = 0; changed = true
        end
    end
    if math.abs(vz) > 0.001 then
        local nz = math.floor(pz + vz + (vz > 0 and 0.31 or -0.31))
        if not is_solid(safe_block(math.floor(px), floor_y, nz)) then
            vz = 0; changed = true
        end
    end

    if changed then
        pcall(function() player:set_velocity(vx, vy, vz) end)
    end
end

local function do_place(bx, by, bz, player, inv, block_slot, rot,
                        px, py, pz, yaw, pitch, on_ground)
    local ok_sel, orig_slot = pcall(function() return inv:selected_slot() end)
    if not ok_sel then return end

    if block_slot ~= orig_slot then
        pcall(function() inv:set_selected_slot(block_slot) end)
    end

    local old_pitch = nil
    if rot == "Legit" then
        local ok_p, p = pcall(function() return player:pitch() end)
        if ok_p then old_pitch = p end
        pcall(function() player:set_pitch(80) end)
    elseif rot == "Silent" then
        local ok_spoof, spoof = pcall(anemoia.create_posrot_packet,
            px, py, pz, yaw, 80.0, on_ground)
        if ok_spoof and spoof then pcall(mc.send_packet, spoof) end
    end

    for _, n in ipairs(NEIGHBORS) do
        local nx, ny, nz = bx + n.dx, by + n.dy, bz + n.dz
        if is_solid(safe_block(nx, ny, nz)) then
            pcall(mc.place_block, nx, ny, nz, n.face)
            break
        end
    end

    if rot == "Legit" and old_pitch ~= nil then
        pcall(function() player:set_pitch(old_pitch) end)
    elseif rot == "Silent" then
        local ok_r, restore = pcall(anemoia.create_posrot_packet,
            px, py, pz, yaw, pitch, on_ground)
        if ok_r and restore then pcall(mc.send_packet, restore) end
    end

    if block_slot ~= orig_slot then
        pcall(function() inv:set_selected_slot(orig_slot) end)
    end
end

function module:on_tick()
    -- Rate limit: run at ~20fps max regardless of render FPS
    local t = mc.clock()
    local mode = self.settings.mode
    local min_interval = mode == "GodBridge" and 0.033 or 0.05
    if t - self._last_t < min_interval then return end
    self._last_t = t
    self._frame = self._frame + 1

    local player = mc.player()
    if not player then return end

    local w     = mc.is_key_down(87)
    local a     = mc.is_key_down(65)
    local s     = mc.is_key_down(83)
    local d     = mc.is_key_down(68)
    local shift = mc.is_key_down(340)
    if not (w or a or s or d) then return end

    local ok_pos, px, py, pz = pcall(function()
        return player:x(), player:y(), player:z()
    end)
    if not ok_pos then return end

    local ok_g, on_ground = pcall(function() return player:on_ground() end)
    local is_on_ground = ok_g and on_ground

    local bx = math.floor(px)
    -- ceil(py)-1: correct for both on-ground (py integer) and mid-air (py fractional)
    -- floor(py)-1 is wrong when falling — it targets one block too low
    local by = math.ceil(py) - 1
    local bz = math.floor(pz)

    if mode == "Normal" and self.settings.safewalk and is_on_ground then
        apply_safewalk(player, px, py, pz)
    end

    if is_solid(safe_block(bx, by, bz)) then return end

    local ok_inv, inv = pcall(function() return player:inventory() end)
    if not ok_inv or not inv then return end

    -- Cache hotbar block slot; re-scan every 10 frames or when nil
    local block_slot
    if self.settings.hotbar then
        if not self._slot_cache or self._frame - self._slot_cache_age > 4 then
            self._slot_cache = find_block_slot(inv)
            self._slot_cache_age = self._frame
        end
        block_slot = self._slot_cache
    else
        local ok_sel, cur = pcall(function() return inv:selected_slot() end)
        if not ok_sel then return end
        local ok_item, item = pcall(function() return player:main_hand_item() end)
        if ok_item and item then
            local ok_chk, empty, itid = pcall(function() return item:is_empty(), item:type_id() end)
            if ok_chk and not empty and itid:find("block") then block_slot = cur end
        end
    end
    if not block_slot then return end

    local yaw, pitch = 0.0, 0.0
    local ok_rot, yw, pt = pcall(function() return player:yaw(), player:pitch() end)
    if ok_rot then yaw = yw; pitch = pt end

    local rot = self.settings.rotations

    if mode == "Normal" then
        local delay = math.max(1, math.floor(self.settings.delay))
        if self._frame % delay ~= 0 then return end
        do_place(bx, by, bz, player, inv, block_slot, rot,
                 px, py, pz, yaw, pitch, is_on_ground)

    elseif mode == "Legit" then
        if not shift then return end
        local delay = math.max(1, math.floor(self.settings.delay))
        if self._frame % delay ~= 0 then return end
        do_place(bx, by, bz, player, inv, block_slot, rot,
                 px, py, pz, yaw, pitch, is_on_ground)

    elseif mode == "GodBridge" then
        if not w then return end
        pcall(function() player:set_sprinting(true) end)
        do_place(bx, by, bz, player, inv, block_slot, rot,
                 px, py, pz, yaw, pitch, is_on_ground)
    end
end

function module:on_disable()
    self._frame = 0
    self._slot_cache = nil
    self._last_t = 0
end

anemoia.register(module)
