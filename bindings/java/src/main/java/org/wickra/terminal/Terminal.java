package org.wickra.terminal;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.ref.Cleaner;
import java.lang.ref.Reference;

/**
 * A trading-terminal instance driven by JSON commands, over the Wickra C ABI
 * (FFM/Panama). Build one from a JSON config, drive it with command JSON and
 * read back the frame JSON — the same protocol as the native TUI and every other
 * binding.
 */
public final class Terminal implements AutoCloseable {
    private final MemorySegment handle;
    private final Cleaner.Cleanable cleanable;
    private boolean closed;

    /** Build a terminal from a JSON config string. */
    public Terminal(String configJson) {
        MemorySegment created;
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment config = arena.allocateFrom(configJson);
            created = (MemorySegment) Native.NEW.invokeExact(config);
        } catch (RuntimeException | Error e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
        if (created.address() == 0) {
            throw new IllegalArgumentException("wickra-terminal: invalid config");
        }
        this.handle = created;
        this.cleanable = Native.register(this, created);
    }

    /**
     * The handle, refusing a closed terminal.
     *
     * <p>Reading the field directly would pass a freed pointer to the ABI after
     * {@link #close()}; this turns that into an exception the caller can read.
     */
    private MemorySegment handle() {
        if (closed) {
            throw new IllegalStateException("terminal is closed");
        }
        return handle;
    }

    /** Apply a command JSON and return the resulting frame JSON. */
    public String command(String cmdJson) {
        MemorySegment live = handle();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment cmd = arena.allocateFrom(cmdJson);
            MemorySegment outHolder = arena.allocate(ValueLayout.ADDRESS);
            int code = (int) Native.COMMAND.invokeExact(live, cmd, outHolder);
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
        } finally {
            // The handle is in a register by now and nothing else refers to this
            // terminal, so the cleaner could otherwise free it mid-call. The
            // fence is the FFM equivalent of Go's runtime.KeepAlive.
            Reference.reachabilityFence(this);
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

    /** Free the native terminal handle. Idempotent. */
    @Override
    public void close() {
        if (closed) {
            return;
        }
        closed = true;
        // Through the cleanable, not a second direct free: the cleaner would
        // otherwise still hold a registration for a handle that is already gone.
        cleanable.clean();
    }
}
