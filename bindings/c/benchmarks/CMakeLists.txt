# Build the C and C++ throughput benchmarks against the wickra-terminal C ABI.
#
#   cargo build -p wickra-terminal-c --release
#   cmake -S bindings/c/benchmarks -B bindings/c/benchmarks/build
#   cmake --build bindings/c/benchmarks/build --config Release
#   ./bindings/c/benchmarks/build/throughput     [ticks]
#   ./bindings/c/benchmarks/build/throughput_cpp [ticks]
#
# One project for both languages, because the C++ surface is a header inside the
# C binding -- `include/wickra_terminal.hpp` wraps `include/wickra_terminal.h`.
# Two projects meant two copies of the library lookup and the Windows DLL copy,
# and let the pair drift; measured against the same library, the difference
# between the two rows is what the ownership layer costs and nothing else.
#
# WICKRA_TERMINAL_LIB_DIR defaults to the workspace target/release directory and
# can be overridden for an out-of-tree build.

cmake_minimum_required(VERSION 3.15)
project(wickra_terminal_c_benchmark C CXX)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(WICKRA_TERMINAL_LIB_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../../target/release"
    CACHE PATH "Directory containing the built wickra-terminal C ABI library")
set(WICKRA_TERMINAL_INCLUDE "${CMAKE_CURRENT_SOURCE_DIR}/../include")

find_library(WICKRA_TERMINAL_LIB
    NAMES wickra_terminal libwickra_terminal
    HINTS "${WICKRA_TERMINAL_LIB_DIR}"
    REQUIRED)

add_executable(throughput throughput.c)
add_executable(throughput_cpp throughput.cpp)

foreach(target throughput throughput_cpp)
    target_include_directories(${target} PRIVATE "${WICKRA_TERMINAL_INCLUDE}")
    target_link_libraries(${target} PRIVATE "${WICKRA_TERMINAL_LIB}")

    # On Windows the runtime DLL must sit beside the executable; there is no rpath.
    if(WIN32)
        add_custom_command(TARGET ${target} POST_BUILD
            COMMAND ${CMAKE_COMMAND} -E copy_if_different
                "${WICKRA_TERMINAL_LIB_DIR}/wickra_terminal.dll"
                "$<TARGET_FILE_DIR:${target}>"
            VERBATIM)
    endif()
endforeach()
