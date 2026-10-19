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
)

if(WIN32)
    add_executable(sd360 WIN32 ${SD360_SOURCES})
else()
    add_executable(sd360 ${SD360_SOURCES})
endif()

rexglue_setup_target(sd360)
