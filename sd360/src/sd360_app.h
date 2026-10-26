// sd360 - ReXGlue Recompiled Project
//
// Customize your app by overriding virtual hooks from rex::ReXApp.

#pragma once

#include <rex/rex_app.h>
#include <rex/cvar.h>
#include <rex/runtime.h>
#include <rex/system/interfaces/graphics.h>
#include <rex/ui/presenter.h>

#include <atomic>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <thread>

class Sd360App : public rex::ReXApp {
 public:
  using rex::ReXApp::ReXApp;

  static std::unique_ptr<rex::ui::WindowedApp> Create(
      rex::ui::WindowedAppContext& ctx) {
    return std::unique_ptr<Sd360App>(new Sd360App(ctx, "sd360",
        PPCImageConfig));
  }

  // Enable Xenos GPU emulation. Empty gpu_plugin means no GPU emulation, and
  // the runtime then logs "no GPU emulation loaded (gpu_plugin not set)" and
  // ignores every Vd* kernel call, so nothing is ever presented.
  void OnPreSetup(rex::RuntimeConfig& config) override {
    config.gpu_plugin = "xenos";
  }

  // Note: setting vulkan_async_skip_incomplete_frames from
// rex::cvar::SetFlagByName in this hook has no effect -- the flag is still read
// as true at present time and frames are still skipped. It is set via the
// environment in run.sh instead (REX_VULKAN_ASYNC_SKIP_INCOMPLETE_FRAMES=false),
// which does work, because ApplyEnvironment runs later. Left here as a record of
// the attempted in-code fix rather than shipping a no-op override.

  // Debug aid: when SD360_CAPTURE_DIR is set, dump the guest's presented frame
  // (what the game itself drew, not the desktop) to <dir>/frame_<n>.ppm every
  // SD360_CAPTURE_EVERY seconds (default 2). Off unless the variable is set.
  void OnPostLaunchModule(rex::system::XThread* thread) override {
    (void)thread;
    const char* dir = std::getenv("SD360_CAPTURE_DIR");
    if (!dir || !*dir) return;
    const char* every_env = std::getenv("SD360_CAPTURE_EVERY");
    int every = every_env ? std::atoi(every_env) : 2;
    if (every <= 0) every = 2;
    // SD360_CAPTURE_MS=<ms> instead captures every <ms> milliseconds at 1/4
    // size (frame_<ms>.ppm) and logs each frame's mean brightness, for catching
    // single-frame glitches such as black flashes.
    const char* ms_env = std::getenv("SD360_CAPTURE_MS");
    int interval_ms = ms_env ? std::atoi(ms_env) : 0;
    if (interval_ms > 0) {
      capture_dir_ = dir;
      capture_thread_ = std::thread([this, interval_ms]() {
        auto start = std::chrono::steady_clock::now();
        FILE* lumlog = std::fopen((capture_dir_ + "/luma.txt").c_str(), "w");
        while (!stop_capture_.load()) {
          std::this_thread::sleep_for(std::chrono::milliseconds(interval_ms));
          auto* gs = runtime() ? runtime()->graphics_system() : nullptr;
          auto* presenter = gs ? gs->presenter() : nullptr;
          if (!presenter) continue;
          rex::ui::RawImage img;
          if (!presenter->CaptureGuestOutput(img) || !img.width || !img.height) continue;
          long t = long(std::chrono::duration_cast<std::chrono::milliseconds>(
                            std::chrono::steady_clock::now() - start).count());
          uint32_t w = img.width / 4, h = img.height / 4;
          uint64_t sum = 0, n = 0;
          std::string path = capture_dir_ + "/frame_" + std::to_string(t) + ".ppm";
          FILE* f = std::fopen(path.c_str(), "wb");
          if (f) std::fprintf(f, "P6\n%u %u\n255\n", w, h);
          for (uint32_t y = 0; y < h; ++y) {
            const uint8_t* row = img.data.data() + (y * 4) * img.stride;
            for (uint32_t x = 0; x < w; ++x) {
              const uint8_t* p = row + (x * 4) * 4;
              if (f) std::fwrite(p, 1, 3, f);
              // Middle band only, away from the menu bars.
              if (y > h / 8 && y < h * 3 / 4) { sum += p[0] + p[1] + p[2]; n += 3; }
            }
          }
          if (f) std::fclose(f);
          if (lumlog) {
            std::fprintf(lumlog, "%ld %.1f", t, n ? double(sum) / n : 0.0);
            // SD360_PEEK=addr,addr,... (guest physical, hex): log the first
            // 8 bytes at each, e.g. the auto-exposure luminance textures.
            if (const char* peek = std::getenv("SD360_PEEK")) {
              const char* s = peek;
              while (*s) {
                char* end = nullptr;
                unsigned long a = std::strtoul(s, &end, 16);
                if (end == s) break;
                const uint8_t* p = reinterpret_cast<const uint8_t*>(0x100000000ull + a);
                std::fprintf(lumlog, " %08lx:", a);
                for (int k = 0; k < 8; ++k) std::fprintf(lumlog, "%02x", p[k]);
                s = (*end == ',') ? end + 1 : end;
              }
            }
            std::fprintf(lumlog, "\n");
            std::fflush(lumlog);
          }
        }
        if (lumlog) std::fclose(lumlog);
      });
      return;
    }
    capture_dir_ = dir;
    capture_thread_ = std::thread([this, every]() {
      for (int n = 1; !stop_capture_.load(); ++n) {
        for (int i = 0; i < every * 10 && !stop_capture_.load(); ++i)
          std::this_thread::sleep_for(std::chrono::milliseconds(100));
        if (stop_capture_.load()) break;
        auto* gs = runtime() ? runtime()->graphics_system() : nullptr;
        auto* presenter = gs ? gs->presenter() : nullptr;
        if (!presenter) continue;
        rex::ui::RawImage img;
        if (!presenter->CaptureGuestOutput(img) || !img.width || !img.height) {
          std::fprintf(stderr, "[capture] t=%ds no guest output\n", n * every);
          continue;
        }
        std::string path = capture_dir_ + "/frame_" + std::to_string(n * every) + ".ppm";
        if (FILE* f = std::fopen(path.c_str(), "wb")) {
          std::fprintf(f, "P6\n%u %u\n255\n", img.width, img.height);
          for (uint32_t y = 0; y < img.height; ++y) {
            const uint8_t* row = img.data.data() + y * img.stride;
            for (uint32_t x = 0; x < img.width; ++x) std::fwrite(row + x * 4, 1, 3, f);
          }
          std::fclose(f);
        }
      }
    });
  }

  void OnShutdown() override {
    stop_capture_.store(true);
    if (capture_thread_.joinable()) capture_thread_.join();
  }

  // Override virtual hooks for customization:
  // void OnPostInitLogging() override {}
  // void OnLoadXexImage(std::string& xex_image) override {}
  // void OnPostLoadXexImage() override {}
  // void OnPostSetup() override {}
  // void OnCreateDialogs(rex::ui::ImGuiDrawer* drawer) override {}
  // std::unique_ptr<rex::ui::ImGuiDialog> CreateAchievementsOverlay() override;
  // std::unique_ptr<rex::ui::AchievementNotificationDialog>
  // CreateAchievementNotificationDialog() override;
  // void OnConfigurePaths(rex::PathConfig& paths) override {}

 private:
  std::string capture_dir_;
  std::thread capture_thread_;
  std::atomic<bool> stop_capture_{false};
};
