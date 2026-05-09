local module = {
    name = "BlockIn",
    description = "Instantly surrounds you with blocks when triggered",
    category = "World",
    enabled = false,
    settings = {
        walls      = true,
        roof       = true,
        rotations  = "Silent",
    },
    _settings_meta = {
        rotations = { type = "enum", options = { "Off", "Silent" } },
    },
    _fired = false,
}

-- Positions to fill relative to player block (bx, by, bz)
-- dy=0,1 = walls at foot/chest level; dy=2 = roof
local WALL_OFFSETS = {
    { dx=1,  dy=0, dz=0  },
    { dx=-1, dy=0, dz=0  },
    { dx=0,  dy=0, dz=1  },
    { dx=0,  dy=0, dz=-1 },
    { dx=1,  dy=1, dz=0  },
    { dx=-1, dy=1, dz=0  },
    { dx=0,  dy=1, dz=1  },
    { dx=0,  dy=1, dz=-1 },
}
local ROOF_OFFSET = { dx=0, dy=2, dz=0 }

-- Tries to place a block at world position (bx,by,bz) by finding a solid neighbor
local function try_place_at(bx, by, bz)
    local FACES = {
        { dx=-1, dy=0,  dz=0,  face="EAST"  },
        { dx=1,  dy=0,  dz=0,  face="WEST"  },
        { dx=0,  dy=-1, dz=0,  face="UP"    },
        { dx=0,  dy=1,  dz=0,  face="DOWN"  },
        { dx=0,  dy=0,  dz=-1, face="SOUTH" },
        { dx=0,  dy=0,  dz=1,  face="NORTH" },
    }
    for _, f in ipairs(FACES) do
        local nx, ny, nz = bx + f.dx, by + f.dy, bz + f.dz
        local ok, blk = pcall(mc.block, nx, ny, nz)
        if ok and blk then
            local ok2, tid = pcall(function() return blk:type_id() end)
            if ok2 and tid ~= "block.minecraft.air" then
                pcall(mc.place_block, nx, ny, nz, f.face)
                return true
            end
        end
    end
    return false
end

local function find_block_slot(inv)
    for s = 0, 8 do
        local ok, item = pcall(function() return inv:item_at(s) end)
        if ok and item then
            local ok2, empty, tid = pcall(function() return item:is_empty(), item:type_id() end)
            if ok2 and not empty and tid:find("block") then return s end
        end
    end
    return nil
end

function module:on_enable()
    self._fired = false
end

function module:on_tick()
    if self._fired then
        self.enabled = false
        return
    end
    self._fired = true

    local player = mc.player()
    if not player then self.enabled = false; return end

    local ok_pos, px, py, pz = pcall(function() return player:x(), player:y(), player:z() end)
    if not ok_pos then return end

    local inv = player:inventory()
    local block_slot = find_block_slot(inv)
    if not block_slot then return end

    local ok_sel, orig_slot = pcall(function() return inv:selected_slot() end)
    if not ok_sel then return end
    inv:set_selected_slot(block_slot)

    local bx = math.floor(px)
    local by = math.floor(py)
    local bz = math.floor(pz)

    local yaw   = 0.0
    local pitch = 0.0
    local on_g  = false
    pcall(function()
        yaw   = player:yaw()
        pitch = player:pitch()
        on_g  = player:on_ground()
    end)

    -- Place all wall blocks
    if self.settings.walls then
        for _, off in ipairs(WALL_OFFSETS) do
            local tx, ty, tz = bx + off.dx, by + off.dy, bz + off.dz
            local ok_blk, blk = pcall(mc.block, tx, ty, tz)
            if ok_blk and blk then
                local ok_t, tid = pcall(function() return blk:type_id() end)
                if ok_t and tid == "block.minecraft.air" then
                    if self.settings.rotations == "Silent" then
                        local ok_s, spoof = pcall(anemoia.create_posrot_packet, px, py, pz, yaw, 89.9, on_g)
                        if ok_s and spoof then pcall(mc.send_packet, spoof) end
                    end
                    try_place_at(tx, ty, tz)
                end
            end
        end
    end

    -- Place roof
    if self.settings.roof then
        local tx, ty, tz = bx + ROOF_OFFSET.dx, by + ROOF_OFFSET.dy, bz + ROOF_OFFSET.dz
        local ok_blk, blk = pcall(mc.block, tx, ty, tz)
        if ok_blk and blk then
            local ok_t, tid = pcall(function() return blk:type_id() end)
            if ok_t and tid == "block.minecraft.air" then
                try_place_at(tx, ty, tz)
            end
        end
    end

    -- Restore rotation and slot
    if self.settings.rotations == "Silent" then
        local ok_r, restore = pcall(anemoia.create_posrot_packet, px, py, pz, yaw, pitch, on_g)
        if ok_r and restore then pcall(mc.send_packet, restore) end
    end
    inv:set_selected_slot(orig_slot)
end

anemoia.register(module)
