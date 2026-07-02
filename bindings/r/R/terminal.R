#' The wickra-terminal library version.
#' @return A version string.
#' @export
wkterm_version <- function() {
  .Call(C_wkterm_version)
}

#' Build a terminal from a JSON config string.
#' @param config_json A JSON config string.
#' @return A `wickra_terminal` handle (an external pointer).
#' @export
wkterm_new <- function(config_json) {
  .Call(C_wkterm_new, config_json)
}

#' Apply a command JSON and return the resulting frame JSON.
#' @param terminal A terminal handle from [wkterm_new()].
#' @param cmd_json A command JSON string.
#' @return The resulting frame as a JSON string.
#' @export
wkterm_command <- function(terminal, cmd_json) {
  .Call(C_wkterm_command, terminal, cmd_json)
}
