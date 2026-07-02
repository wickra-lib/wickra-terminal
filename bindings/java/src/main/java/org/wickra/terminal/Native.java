package org.wickra.terminal;

import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.nio.file.Path;

/** Raw FFM (Panama) downcall surface for the wickra-terminal C ABI. */
final class Native {
    private Native() {}

    static final int OK = 0;

    private static final Linker LINKER = Linker.nativeLinker();
    private static final Arena LIB_ARENA = Arena.ofShared();
    private static final SymbolLookup LOOKUP = loadLibrary();

    static final ValueLayout.OfInt C_INT = ValueLayout.JAVA_INT;
    static final java.lang.foreign.AddressLayout C_PTR = ValueLayout.ADDRESS;

    static final MethodHandle NEW =
            handle("wickra_terminal_new", FunctionDescriptor.of(C_PTR, C_PTR));
    static final MethodHandle FREE =
            handle("wickra_terminal_free", FunctionDescriptor.ofVoid(C_PTR));
    static final MethodHandle COMMAND =
            handle("wickra_terminal_command", FunctionDescriptor.of(C_INT, C_PTR, C_PTR, C_PTR));
    static final MethodHandle FREE_STRING =
            handle("wickra_terminal_free_string", FunctionDescriptor.ofVoid(C_PTR));
    static final MethodHandle VERSION =
            handle("wickra_terminal_version", FunctionDescriptor.of(C_PTR));

    private static SymbolLookup loadLibrary() {
        String dir = System.getProperty("native.lib.dir");
        String libFile = System.mapLibraryName("wickra_terminal");
        Path path = dir != null ? Path.of(dir, libFile) : Path.of(libFile);
        return SymbolLookup.libraryLookup(path, LIB_ARENA);
    }

    private static MethodHandle handle(String name, FunctionDescriptor descriptor) {
        MemorySegment symbol = LOOKUP.find(name)
                .orElseThrow(() -> new IllegalStateException("missing C ABI symbol: " + name));
        return LINKER.downcallHandle(symbol, descriptor);
    }
}
