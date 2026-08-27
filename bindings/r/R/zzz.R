.onLoad <- function(libname, pkgname) {
  # On Windows the package object depends on the bundled C ABI
  # wickra_terminal.dll, and the loader searches PATH for it, so the package's
  # own libs directory has to be on PATH before the object is loaded. That is
  # also why NAMESPACE carries no `useDynLib`: it would load the object during
  # namespace loading, which happens before this hook runs.
  #
  # On Linux and macOS the rpath baked in by configure ($ORIGIN /
  # @loader_path) locates the library, so no PATH change is needed.
  if (.Platform$OS.type == "windows") {
    libs <- system.file(paste0("libs", .Platform$r_arch),
                        package = pkgname, lib.loc = libname)
    if (nzchar(libs)) {
      Sys.setenv(PATH = paste(libs, Sys.getenv("PATH"), sep = .Platform$path.sep))
    }
  }
  library.dynam("wickraterminal", pkgname, libname)
}

.onUnload <- function(libpath) {
  library.dynam.unload("wickraterminal", libpath)
}
