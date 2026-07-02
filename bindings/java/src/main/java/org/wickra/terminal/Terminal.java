package org.wickra.terminal;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;

/**
 * A trading-terminal instance driven by JSON commands, over the Wickra C ABI
 * (FFM/Panama). Build one from a JSON config, drive it with command JSON and
 * read back the frame JSON — the same protocol as the native TUI and every other
 * binding.
 */
public final class Terminal implements AutoCloseable {
    private MemorySegment handle;

    /** Build a terminal from a JSON config string. */
    public Terminal(String configJson) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment config = arena.allocateFrom(configJson);
            MemorySegment created = (MemorySegment) Native.NEW.invokeExact(config);
            if (created.address() == 0) {
                throw new IllegalArgumentException("wickra-terminal: invalid config");
            }
            this.handle = created;
        } catch (RuntimeException | Error e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /** Apply a command JSON and return the resulting frame JSON. */
    public String command(String cmdJson) {
        if (handle == null) {
            throw new IllegalStateException("terminal is closed");
        }
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment cmd = arena.allocateFrom(cmdJson);
            MemorySegment outHolder = arena.allocate(ValueLayout.ADDRESS);
            int code = (int) Native.COMMAND.invokeExact(handle, cmd, outHolder);
            MemorySegment outPtr = outHolder.get(ValueLayout.ADDRESS, 0);
            String result = "";
            if (outPtr.address() != 0) {
                result = outPtr.reinterpret(Long.MAX_VALUE).getString(0);
                Native.FREE_STRING.invokeExact(outPtr);
            }
            if (code != Native.OK) {
                throw new IllegalStateException("wickra-terminal: " + result);
            }
            return result;
        } catch (RuntimeException | Error e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /** The library version. */
    public static String version() {
        try {
            MemorySegment ptr = (MemorySegment) Native.VERSION.invokeExact();
            return ptr.reinterpret(Long.MAX_VALUE).getString(0);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /** Free the native terminal handle. */
    @Override
    public void close() {
        if (handle != null) {
            try {
                Native.FREE.invokeExact(handle);
            } catch (Throwable t) {
                throw new RuntimeException(t);
            }
            handle = null;
        }
    }
}
