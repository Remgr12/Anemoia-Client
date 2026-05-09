local module = {
    name = "HitSelect",
    description = "Times attacks to i-frame windows for maximum knockback advantage",
    category = "Combat",
    enabled = false,
    settings = {
        pause_duration = 200,   -- max ms to hold queued attack before forcing release
        wait_for_hit   = true,  -- release queued attack when we take damage (counter-hit)
        burst_mode     = false, -- only attack while NOT in our own i-frames
    },
    _settings_meta = {
        pause_duration = { min = 50, max = 450 },
    },
    _queued    = false,
    _queue_t   = 0,
    _was_hurt  = false,
    _release   = false,  -- set true this tick to signal a release
}

-- Exposed globally so KillAura and SilentAura can gate attacks through this module
_G["hitselect_module"] = module

-- Combat modules call this instead of attacking directly.
-- Returns true when the attack should fire now, false to hold.
function module:should_attack(player)
    if not self.enabled then return true end

    local ok_ht, hurt_time = pcall(function() return player:hurt_time() end)

    -- Burst mode: skip while in our own i-frames
    if self.settings.burst_mode and ok_ht and hurt_time > 0 then
        return false
    end

    -- If a release was signalled this tick (we just took a hit), fire
    if self._release then
        self._release = false
        return true
    end

    -- Queue the attack if not already queued
    local now = os.clock() * 1000
    if not self._queued then
        self._queued  = true
        self._queue_t = now
        return false
    end

    -- Force release after max pause duration
    if now - self._queue_t >= self.settings.pause_duration then
        self._queued = false
        return true
    end

    return false
end

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local ok_ht, hurt_time = pcall(function() return player:hurt_time() end)
    if not ok_ht then return end

    local just_hit = hurt_time > 0 and not self._was_hurt
    self._was_hurt = hurt_time > 0

    -- Signal attack release when we take damage (counter-hit window)
    if just_hit and self.settings.wait_for_hit and self._queued then
        self._release = true
        self._queued  = false
    end
end

function module:on_disable()
    self._queued   = false
    self._release  = false
    self._was_hurt = false
end

anemoia.register(module)
