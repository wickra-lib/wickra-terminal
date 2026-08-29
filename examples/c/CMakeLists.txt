# Build and run the C and C++ examples against the wickra-terminal C ABI.
#
#   cargo build --release -p wickra-terminal-c
#   cmake -S examples/c -B examples/c/build
#   cmake --build examples/c/build --config Release
#   ctest --test-dir examples/c/build -C Release --output-on-failure
#
# WICKRA_TERMINAL_LIB_DIR defaults to the workspace target/release directory and
# can be overridden for out-of-tree builds.

cmake_minimum_required(VERSION 3.15)
project(wickra_terminal_examples C CXX)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
# 17, because wickra_terminal.hpp uses a nested namespace and [[nodiscard]].
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(WICKRA_TERMINAL_LIB_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../target/release"
    CACHE PATH "Directory containing the built wickra-terminal C ABI library")

set(WICKRA_TERMINAL_INCLUDE "${CMAKE_CURRENT_SOURCE_DIR}/../../bindings/c/include")

find_library(WICKRA_TERMINAL_LIB
    NAMES wickra_terminal libwickra_terminal
    HINTS "${WICKRA_TERMINAL_LIB_DIR}"
    REQUIRED)

add_executable(synth synth.c)
target_include_directories(synth PRIVATE "${WICKRA_TERMINAL_INCLUDE}")
target_link_libraries(synth PRIVATE "${WICKRA_TERMINAL_LIB}")

add_executable(terminal terminal.cpp)
target_include_directories(terminal PRIVATE "${WICKRA_TERMINAL_INCLUDE}")
target_link_libraries(terminal PRIVATE "${WICKRA_TERMINAL_LIB}")

# Cross-language golden parity, so the C ABI hub is held to the same corpus as
# the eight language surfaces that sit on it.
add_executable(golden golden.c)
target_include_directories(golden PRIVATE "${WICKRA_TERMINAL_INCLUDE}")
target_link_libraries(golden PRIVATE "${WICKRA_TERMINAL_LIB}")

# The same corpus through the C++ wrapper. It calls the same five functions,
# so what this proves is narrow and worth proving: that the ownership layer
# does not change what comes back.
add_executable(golden_cpp golden.cpp)
target_include_directories(golden_cpp PRIVATE "${WICKRA_TERMINAL_INCLUDE}")
target_link_libraries(golden_cpp PRIVATE "${WICKRA_TERMINAL_LIB}")

# The ownership rules the C++ header exists to enforce.
add_executable(lifetime lifetime.cpp)
target_include_directories(lifetime PRIVATE "${WICKRA_TERMINAL_INCLUDE}")
target_link_libraries(lifetime PRIVATE "${WICKRA_TERMINAL_LIB}")

# On Windows the runtime DLL must sit next to the executable (there is no
# rpath); without it the loader pops a modal "DLL not found" dialog that hangs
# ctest. Copy it next to each example after build. (Unix resolves the .so/.dylib
# via the linker's embedded path, so this is Windows-only.)
if(WIN32)
    foreach(example synth terminal golden golden_cpp lifetime)
        add_custom_command(TARGET ${example} POST_BUILD
            COMMAND ${CMAKE_COMMAND} -E copy_if_different
                "${WICKRA_TERMINAL_LIB_DIR}/wickra_terminal.dll"
                "$<TARGET_FILE_DIR:${example}>"
            VERBATIM)
    endforeach()
endif()

enable_testing()
add_test(NAME synth COMMAND synth)
add_test(NAME terminal COMMAND terminal)
# The golden test walks up from its working directory to find golden/, so run it
# from the repository root rather than the build tree.
add_test(NAME golden COMMAND golden WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/../..")
add_test(NAME golden_cpp COMMAND golden_cpp WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/../..")
add_test(NAME lifetime COMMAND lifetime)
# A missing runtime dependency must fail fast, never block the runner for hours.
set_tests_properties(synth terminal golden golden_cpp lifetime PROPERTIES TIMEOUT 60)
