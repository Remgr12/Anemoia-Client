local module = {
    name        = "Scaffold",
    description = "Automatically places blocks under your feet",
    category    = "World",
    enabled     = false,
    settings = {
        rotations = true,
        delay     = 2,
    },
    _settings_meta = {
        rotations = { type = "boolean" },
        delay     = { type = "number", min = 0, max = 10 },
    },
    _tick = 0,
}

local Y_MIN, Y_MAX = -64, 320

-- Ordered neighbor offsets: below first (most common), then sides.
-- face is the face of the NEIGHBOR block to click to fill the target air slot.
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

function module:on_tick()
    self._tick = self._tick + 1
    local delay = math.max(1, math.floor(self.settings.delay))
    if self._tick % delay ~= 0 then return end

    local player = mc.player()
    if not player then return end

    local ok, px, py, pz = pcall(function()
        return player:x(), player:y(), player:z()
    end)
    if not ok then return end

    -- block coordinate of the slot directly under the player's feet
    local bx = math.floor(px)
    local by = math.floor(py) - 1
    local bz = math.floor(pz)

    -- only act when that slot is air
    if is_solid(safe_block(bx, by, bz)) then return end

    -- must have a block item in hand
    local ok_item, item = pcall(function() return player:main_hand_item() end)
    if not ok_item or not item then return end

    local ok_chk, empty, tid = pcall(function()
        return item:is_empty(), item:type_id()
    end)
    if not ok_chk or empty then return end
    if not tid:find("block") then return end

    -- rotate looking down so server sees a reasonable rotation
    if self.settings.rotations then
        pcall(function() player:set_pitch(80) end)
    end

    -- find a solid neighbor face and place against it
    for _, n in ipairs(NEIGHBORS) do
        local nx, ny, nz = bx + n.dx, by + n.dy, bz + n.dz
        local ntid = safe_block(nx, ny, nz)
        if is_solid(ntid) then
            pcall(mc.place_block, nx, ny, nz, n.face)
            return
        end
    end
end

function module:on_disable()
    -- restore pitch when disabling
    local player = mc.player()
    if player then
        pcall(function() player:set_pitch(0) end)
    end
end

anemoia.register(module)
