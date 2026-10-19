# sd360 - ReXGlue Recompiled Project
#
# SDK boilerplate lives in generated/rexglue.cmake.

cmake_minimum_required(VERSION 3.25)
project(sd360 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

include(generated/rexglue.cmake)

# Sources
set(SD360_SOURCES
    src/main.cpp
    src/diagnostics.cpp
)

if(WIN32)
    add_executable(sd360 WIN32 ${SD360_SOURCES})
else()
    add_executable(sd360 ${SD360_SOURCES})
endif()

# GPU_PLUGINS xenos: enables Xenos (Xbox 360) GPU emulation. Without it the
# runtime logs "no GPU emulation loaded (gpu_plugin not set)" and ignores every
# Vd* kernel call, so nothing is ever presented.
rexglue_setup_target(sd360 GPU_PLUGINS xenos)
