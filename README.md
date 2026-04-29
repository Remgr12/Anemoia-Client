# Anemoia Client

Minecraft Ghost Client designed for Linux using `.so` payloads.

## Overview

Anemoia Client is a modular Minecraft “ghost client” for Linux. This repository includes client-side functionality and exposes a Lua scripting API for interacting with the client/game state.

## Quick start

> If this repo includes build/install instructions elsewhere (for example in a `docs/` folder or release notes), follow those first.

- Clone:
  ```bash
  git clone https://github.com/Remgr12/Anemoia-Client.git
  cd Anemoia-Client
  ```

## Lua API

This section documents the Lua API exposed by the client. (Content below is preserved from the existing README, with light formatting improvements.)

### Globals

- `mc.player()`: Returns the `LocalPlayer` or `nil`.
- `mc.entities()`: Returns a list of all entities in the world.
- `mc.attack(entity)`: Attacks the specified entity.
- `mc.click()`: Performs a left-click.
- `mc.is_key_down(key)`: Returns `true` if the specified GLFW key/button is pressed.
- `mc.send_packet(packet)`: Sends a packet (triggers `on_packet_send`).
- `mc.block(x, y, z)`: Returns a `LuaBlock`.

### LocalPlayer

- `x()`, `y()`, `z()`: Position coordinates.
- `yaw()`, `pitch()`: Rotation angles.
- `set_yaw(v)`, `set_pitch(v)`: Set rotation angles.
- `velocity()`: Returns `{x, y, z}`.
- `set_velocity(x, y, z)`: Set movement velocity.
- `on_ground()`: Returns `true` if on ground.
- `set_on_ground(bool)`: Spoof ground status.
- `is_sprinting()`, `set_sprinting(bool)`: Sprint control.
- `fall_distance()`: Current fall distance.
- `hurt_time()`: Current hurt time ticks.
- `input()`: Returns a table with `{up, down, left, right, jumping, sneaking}`.
- `inventory()`: Returns `LuaInventory`.

### Events & Hooks

- `anemoia.on_render(function(painter) ... end)`: Called every frame for 2D drawing.
- `anemoia.on_packet_send(function(packet) ... end)`: Called before a packet is sent. Return `true` to cancel the packet.

### Packet

- `type_name()`: Returns the Java class name of the packet.

### Module registration

Modules are registered via `anemoia.register(table)`. 

- Keybinds can be set in the ClickGUI (Right Shift) using the **Bind** button.
- Settings can be defined in the `settings` table and ranges in `_settings_meta`.
