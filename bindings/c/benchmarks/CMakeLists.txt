# Build the C throughput benchmark against the wickra-terminal C ABI.
#
#   cargo build -p wickra-terminal-c --release
#   cmake -S bindings/c/benchmarks -B bindings/c/benchmarks/build
#   cmake --build bindings/c/benchmarks/build --config Release
#   ./bindings/c/benchmarks/build/throughput [ticks]
#
# WICKRA_TERMINAL_LIB_DIR defaults to the workspace target/release directory and
# can be overridden for an out-of-tree build.

cmake_minimum_required(VERSION 3.15)
project(wickra_terminal_c_benchmark C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

set(WICKRA_TERMINAL_LIB_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../../target/release"
    CACHE PATH "Directory containing the built wickra-terminal C ABI library")
set(WICKRA_TERMINAL_INCLUDE "${CMAKE_CURRENT_SOURCE_DIR}/../include")

find_library(WICKRA_TERMINAL_LIB
    NAMES wickra_terminal libwickra_terminal
    HINTS "${WICKRA_TERMINAL_LIB_DIR}"
    REQUIRED)

add_executable(throughput throughput.c)
target_include_directories(throughput PRIVATE "${WICKRA_TERMINAL_INCLUDE}")
target_link_libraries(throughput PRIVATE "${WICKRA_TERMINAL_LIB}")

# On Windows the runtime DLL must sit beside the executable; there is no rpath.
if(WIN32)
    add_custom_command(TARGET throughput POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
            "${WICKRA_TERMINAL_LIB_DIR}/wickra_terminal.dll"
            "$<TARGET_FILE_DIR:throughput>"
        VERBATIM)
endif()
