# Build the CXX throughput benchmark against the wickra-terminal C ABI.
#
#   cargo build -p wickra-terminal-c --release
#   cmake -S bindings/cpp/benchmarks -B bindings/cpp/benchmarks/build
#   cmake --build bindings/cpp/benchmarks/build --config Release
#   ./bindings/cpp/benchmarks/build/throughput [ticks]
#
# WICKRA_TERMINAL_LIB_DIR defaults to the workspace target/release directory and
# can be overridden for an out-of-tree build.

cmake_minimum_required(VERSION 3.15)
project(wickra_terminal_cpp_benchmark CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(WICKRA_TERMINAL_LIB_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../../target/release"
    CACHE PATH "Directory containing the built wickra-terminal C ABI library")
set(WICKRA_TERMINAL_INCLUDE "${CMAKE_CURRENT_SOURCE_DIR}/../../c/include")

find_library(WICKRA_TERMINAL_LIB
    NAMES wickra_terminal libwickra_terminal
    HINTS "${WICKRA_TERMINAL_LIB_DIR}"
    REQUIRED)

add_executable(throughput throughput.cpp)
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
