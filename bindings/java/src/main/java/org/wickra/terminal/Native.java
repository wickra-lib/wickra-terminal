package org.wickra.terminal;

import java.io.IOException;
import java.io.InputStream;
import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.lang.ref.Cleaner;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

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

    /**
     * Releases the native terminal of a {@link Terminal} that was never closed.
     *
     * <p>Without it {@code close()} was the only release path, so an unclosed
     * terminal held its whole native state — the folded {@code AppState}, every
     * source and any live socket — for the lifetime of the process. Every class
     * in the wickra library registers one of these; this binding registered
     * none.
     */
    private static final Cleaner CLEANER = Cleaner.create();

    static Cleaner.Cleanable register(Object owner, MemorySegment handle) {
        return CLEANER.register(owner, new FreeAction(handle));
    }

    /**
     * The release itself. A record rather than a lambda, deliberately: a lambda
     * capturing the terminal would keep it reachable and the cleaner would never
     * run. This holds the handle and nothing else.
     */
    private record FreeAction(MemorySegment handle) implements Runnable {
        @Override
        public void run() {
            try {
                FREE.invokeExact(handle);
            } catch (Throwable ignored) {
                // Best-effort release during cleaning; nothing here can act on it.
            }
        }
    }

    /**
     * Resolve the native library, in this order: {@code -Dnative.lib.dir=<dir>}
     * (an explicit choice always wins), the copy bundled in the jar under
     * {@code /native/<os>-<arch>/} (extracted to a temporary file -- how a Maven
     * Central consumer gets it), every {@code target/release} or
     * {@code target/debug} found by walking up from the working directory and
     * from this class's own location (a checkout after {@code cargo build}),
     * and finally the bare file name on the loader's own search path.
     */
    private static SymbolLookup loadLibrary() {
        String libFile = System.mapLibraryName("wickra_terminal");
        List<Path> candidates = new ArrayList<>();
        String dir = System.getProperty("native.lib.dir");
        if (dir != null) {
            candidates.add(Path.of(dir, libFile));
        }
        Path bundled = extractBundled(libFile);
        if (bundled != null) {
            candidates.add(bundled);
        }
        candidates.addAll(findInCargoTarget(libFile));
        candidates.add(Path.of(libFile));
        List<String> rejected = new ArrayList<>();
        for (Path candidate : candidates) {
            try {
                return SymbolLookup.libraryLookup(candidate, LIB_ARENA);
            } catch (IllegalArgumentException e) {
                rejected.add(candidate + " (" + e.getMessage() + ")");
            }
        }
        throw new UnsatisfiedLinkError("wickra-terminal: could not load the native library (" + libFile
                + "). Bundle it under resources/native/" + platformDir()
                + "/, pass -Dnative.lib.dir=<dir>, or build the C ABI with"
                + " `cargo build -p wickra-terminal-c --release`."
                + (rejected.isEmpty() ? "" : " Tried: " + String.join("; ", rejected)));
    }

    private static Path extractBundled(String libFile) {
        String resource = "/native/" + platformDir() + "/" + libFile;
        try (InputStream in = Native.class.getResourceAsStream(resource)) {
            if (in == null) {
                return null;
            }
            Path tmp = Files.createTempFile("wickra-terminal-", "-" + libFile);
            tmp.toFile().deleteOnExit();
            Files.copy(in, tmp, StandardCopyOption.REPLACE_EXISTING);
            return tmp;
        } catch (IOException e) {
            return null;
        }
    }

    /**
     * Walk up from the working directory and from this class's own location
     * looking for {@code target/<profile>/<library>}, collecting every hit.
     */
    private static List<Path> findInCargoTarget(String libFile) {
        List<Path> found = new ArrayList<>();
        for (Path base : new Path[] {Paths.get(System.getProperty("user.dir", ".")), codeSourceDir()}) {
            Path dir = base;
            for (int i = 0; i < 16 && dir != null; i++) {
                for (String profile : new String[] {"release", "debug"}) {
                    Path candidate = dir.resolve("target").resolve(profile).resolve(libFile);
                    if (Files.isRegularFile(candidate) && !found.contains(candidate)) {
                        found.add(candidate);
                    }
                }
                dir = dir.getParent();
            }
        }
        return found;
    }

    private static Path codeSourceDir() {
        try {
            Path p = Paths.get(Native.class.getProtectionDomain().getCodeSource().getLocation().toURI());
            return Files.isDirectory(p) ? p : p.getParent();
        } catch (Exception e) {
            return null;
        }
    }

    private static String platformDir() {
        String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        String osName = os.contains("win") ? "win" : (os.contains("mac") || os.contains("darwin")) ? "osx" : "linux";
        String arch = System.getProperty("os.arch", "").toLowerCase(Locale.ROOT);
        String archName = (arch.equals("aarch64") || arch.equals("arm64")) ? "arm64" : "x64";
        return osName + "-" + archName;
    }

    private static MethodHandle handle(String name, FunctionDescriptor descriptor) {
        MemorySegment symbol = LOOKUP.find(name)
                .orElseThrow(() -> new IllegalStateException("missing C ABI symbol: " + name));
        return LINKER.downcallHandle(symbol, descriptor);
    }
}
