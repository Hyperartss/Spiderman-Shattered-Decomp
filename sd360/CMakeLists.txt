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

# Guest modules are dlopen()'d at runtime by bare name -- "<project>_<target>",
# with no "lib" prefix and no ".so" suffix (project_recompiler.cpp sets
# shared_lib_name to projectName + "_" + targetName; kernel_state.cpp dlopen()s
# that string). This glibc does not apply the lib-prefix/.so transform to a bare
# dlopen name -- verified with LD_DEBUG=files, which enumerated no candidate
# paths at all -- so the module fails with "Failed to load shared library for
# module". Expose the built library under the exact name the runtime asks for.
#
# Note the module is dlopen'd from librexruntimed.so, so the search path is that
# object's $ORIGIN plus LD_LIBRARY_PATH. DT_RUNPATH is not inherited for
# dlopen; set LD_LIBRARY_PATH to the build dir when launching.
if(TARGET sd360_GameLogic)
    add_custom_command(TARGET sd360_GameLogic POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E create_symlink
            $<TARGET_FILE:sd360_GameLogic>
            $<TARGET_FILE_DIR:sd360_GameLogic>/sd360_GameLogic
        VERBATIM
    )
endif()
