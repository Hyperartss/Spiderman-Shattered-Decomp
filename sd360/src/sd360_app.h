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
