// Package wickraterminal provides idiomatic Go bindings for wickra-terminal over
// its C ABI hub: build a Terminal from a JSON config, drive it with command JSON
// and read back the frame JSON — the same protocol as the native TUI and every
// other binding.
//
// The binding links the prebuilt C ABI library, staged per platform under
// ./lib/<goos>_<goarch>/, with the header vendored under ./include.
package wickraterminal

/*
#cgo CFLAGS: -I${SRCDIR}/include
#cgo linux,amd64 LDFLAGS: -L${SRCDIR}/lib/linux_amd64 -lwickra_terminal -Wl,-rpath,${SRCDIR}/lib/linux_amd64
#cgo linux,arm64 LDFLAGS: -L${SRCDIR}/lib/linux_arm64 -lwickra_terminal -Wl,-rpath,${SRCDIR}/lib/linux_arm64
#cgo darwin,amd64 LDFLAGS: -L${SRCDIR}/lib/darwin_amd64 -lwickra_terminal -Wl,-rpath,${SRCDIR}/lib/darwin_amd64
#cgo darwin,arm64 LDFLAGS: -L${SRCDIR}/lib/darwin_arm64 -lwickra_terminal -Wl,-rpath,${SRCDIR}/lib/darwin_arm64
#cgo windows,amd64 LDFLAGS: -L${SRCDIR}/lib/windows_amd64 -l:wickra_terminal.dll
#cgo windows,arm64 LDFLAGS: -L${SRCDIR}/lib/windows_arm64 -l:wickra_terminal.dll
#include <stdlib.h>
#include "wickra_terminal.h"
*/
import "C"

import (
	"errors"
	"fmt"
	"runtime"
	"unsafe"
)

// ErrClosed is returned by Command when the terminal has already been closed.
//
// The C ABI rejects a null handle rather than dereferencing it, so this was not
// a crash -- but it returned an error whose message was the empty string the ABI
// had no room to write, which told a caller nothing about what they had done.
var ErrClosed = errors.New("wickra-terminal: terminal is closed")

// Terminal is a trading-terminal instance driven by JSON commands.
//
// A Terminal is not safe for concurrent use. It owns a mutable native terminal,
// so two goroutines calling Command at once is a data race in the core, not a
// contended lock; synchronise externally if more than one goroutine drives it.
type Terminal struct {
	handle *C.WickraTerminal
}

// New builds a terminal from a JSON config string. Call Close when done (a
// finalizer also frees it, but explicit Close is preferred).
func New(configJSON string) (*Terminal, error) {
	cconfig := C.CString(configJSON)
	defer C.free(unsafe.Pointer(cconfig))

	handle := C.wickra_terminal_new(cconfig)
	if handle == nil {
		return nil, fmt.Errorf("wickra-terminal: invalid config")
	}
	t := &Terminal{handle: handle}
	runtime.SetFinalizer(t, (*Terminal).Close)
	return t, nil
}

// Command applies a command JSON and returns the resulting frame JSON.
func (t *Terminal) Command(cmdJSON string) (string, error) {
	if t.handle == nil {
		return "", ErrClosed
	}

	ccmd := C.CString(cmdJSON)
	defer C.free(unsafe.Pointer(ccmd))

	var out *C.char
	code := C.wickra_terminal_command(t.handle, ccmd, &out)
	// The handle was read into an argument register above, after which nothing
	// in this frame refers to t -- so the collector may run its finalizer, and
	// free the terminal, while the call is still executing. KeepAlive holds t
	// live across the call, as every cgo call in the wickra library does.
	runtime.KeepAlive(t)
	result := ""
	if out != nil {
		result = C.GoString(out)
		C.wickra_terminal_free_string(out)
	}
	if code != C.WICKRA_TERMINAL_OK {
		return "", fmt.Errorf("wickra-terminal: %s", result)
	}
	return result, nil
}

// Close frees the terminal handle. Safe to call more than once; a Command after
// it returns ErrClosed.
func (t *Terminal) Close() {
	if t.handle != nil {
		C.wickra_terminal_free(t.handle)
		t.handle = nil
		runtime.SetFinalizer(t, nil)
	}
	runtime.KeepAlive(t)
}

// Version returns the library version.
func Version() string {
	return C.GoString(C.wickra_terminal_version())
}
