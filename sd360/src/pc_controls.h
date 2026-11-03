// Built-in PC (keyboard + mouse) control layout for sd360.
//
// The recompiled game only knows Xbox 360 pad buttons, so each key is bound to
// one of those through the SDK's mnk input driver. These are *defaults*: an
// explicit value from a config file, a REX_* environment variable or a
// --flag on the command line still wins, because Apply() only fills in flags
// that are still at their compiled-in default (Source::kDefault).
//
// Layout reference is the 2010 PC port as documented for the PC release:
//   WASD move, mouse look, LMB fast attack, RMB web zip (tap) / web swing
//   (hold), Space jump, Enter select, Esc pause menu.
// Bindings the PC port docs do not spell out (strong attack, dodge, web shot,
// rage, spider-sense, camera reset) are marked [guess] below and can be
// changed with the matching --keybind_* flag without a rebuild.
//
// Key names must be the ones the SDK parses (see rex/ui/keybinds.cpp):
// A-Z, 0-9, Space, Return, Escape, Tab, Backspace, Up/Down/Left/Right,
// LMB/RMB/MMB, Semicolon, Quote, ... Shift/Ctrl/Alt are modifiers only and
// cannot be bound on their own.

#pragma once

#include <rex/cvar.h>

#include <string_view>

namespace sd360::pc_controls {

struct Binding {
  std::string_view name;
  std::string_view value;
};

// One entry per row in the game's own control screen, plus the two driver
// switches. A comma-separated value means "any of these keys".
inline constexpr Binding kBindings[] = {
    // Driver on, mouse drives the right stick (camera).
    {"mnk_mode", "true"},
    {"mnk_mouse", "true"},
    {"mnk_sensitivity", "1.0"},

    // Movement (left stick) - PC port: WASD.
    {"keybind_lstick_up", "W"},
    {"keybind_lstick_down", "S"},
    {"keybind_lstick_left", "A"},
    {"keybind_lstick_right", "D"},
    // Unused by this game; empty keeps the key free.
    {"keybind_lstick_press", ""},

    // Camera keys off: mnk_mouse turns the camera. [guess] MMB = R3 reset.
    {"keybind_rstick_up", ""},
    {"keybind_rstick_down", ""},
    {"keybind_rstick_left", ""},
    {"keybind_rstick_right", ""},
    {"keybind_rstick_press", "MMB"},

    // Face buttons. A is jump / menu accept (PC port: Space, Enter).
    {"keybind_a", "Space,Return"},
    // B is grab / interact / menu back (PC port: Backspace).
    {"keybind_b", "Backspace,E"},
    // X is fast attack (PC port: left mouse button).
    {"keybind_x", "LMB"},
    // Y is strong attack. [guess] MMB, since LMB/RMB are taken above.
    {"keybind_y", "MMB"},

    // RT: web zip (tap) / web swing (hold) - PC port: RMB.
    {"keybind_right_trigger", "RMB"},
    // LT: dodge. [guess] Q; Shift/Ctrl cannot be bound alone.
    {"keybind_left_trigger", "Q"},
    // RB: web shot. [guess]
    {"keybind_right_shoulder", "R"},
    // LB: rage (Ultimate) / accelerated vision (2099). [guess]
    {"keybind_left_shoulder", "C"},

    // D-pad. Up is spider-sense (2099: accelerated vision). [guess]
    {"keybind_dpad_up", "Up,V"},
    {"keybind_dpad_down", "Down"},
    {"keybind_dpad_left", "Left"},
    {"keybind_dpad_right", "Right"},

    // Start / Back. Esc is both pause and (in menus) back out.
    {"keybind_start", "Escape"},
    {"keybind_back", "Tab"},
};

// Fill in any binding still at its compiled-in default. Safe to call once the
// cvar system is final (ReXApp::OnPostSetup and later).
inline void Apply() {
  for (const Binding& b : kBindings) {
    if (rex::cvar::GetFlagSource(b.name) != rex::cvar::Source::kDefault) {
      continue;
    }
    rex::cvar::SetFlagByName(b.name, b.value);
  }
}

}  // namespace sd360::pc_controls
