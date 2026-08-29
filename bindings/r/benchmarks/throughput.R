# Throughput benchmark for the wickra-terminal R binding.
#
# What this measures is the boundary, not the core. Every binding drives the
# same Rust terminal through one function -- a command JSON in, a frame JSON
# out -- so the number is the cost of crossing this boundary once per command.
# R's row is the one to read with the interpreter in mind: the .Call itself is
# cheap, and the loop around it is not.
#
# Install the package first, then run from the repository root:
#
#   cargo build -p wickra-terminal-c --release
#   WKTERM_INC=bindings/c/include WKTERM_LIB=target/release R CMD INSTALL bindings/r
#   Rscript bindings/r/benchmarks/throughput.R
#   Rscript bindings/r/benchmarks/throughput.R 100000

library(wickraterminal)

# Shared by all nine binding benchmarks, so the numbers compare.
config <- paste0(
  '{"sources":[{"Synth":{"seed":1}}],',
  '"layout":{"panels":[',
  '{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":40}},',
  '{"kind":"Book","rect":{"x":0,"y":40,"w":50,"h":30}},',
  '{"kind":"Tape","rect":{"x":50,"y":40,"w":50,"h":30}}]}}'
)
subscribe <- '{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}'
tick <- '{"type":"Tick"}'
list_cmd <- '{"type":"ListIndicators"}'

args <- commandArgs(trailingOnly = TRUE)
ticks <- if (length(args) > 0) suppressWarnings(as.integer(args[1])) else NA_integer_
if (is.na(ticks) || ticks < 100) ticks <- 20000L

term <- wkterm_new(config)
invisible(wkterm_command(term, subscribe))
frame_bytes <- nchar(wkterm_command(term, tick), type = "bytes")
catalogue_bytes <- nchar(wkterm_command(term, list_cmd), type = "bytes")

# Median of three timed runs, after one warmup.
median_ns <- function(command, count) {
  drive <- function() {
    for (i in seq_len(count)) invisible(wkterm_command(term, command))
  }
  drive()
  samples <- numeric(3)
  for (i in seq_len(3)) {
    start <- Sys.time()
    drive()
    samples[i] <- as.numeric(difftime(Sys.time(), start, units = "secs")) * 1e9
  }
  sort(samples)[2]
}

tick_ns <- median_ns(tick, ticks)
list_ns <- median_ns(list_cmd, 100L)

row <- function(name, count, ns, bytes) {
  cat(sprintf(
    "%-18s%14s%14.2f%11sB\n",
    name,
    formatC(round(count / (ns / 1e9)), format = "d", big.mark = ","),
    ns / count / 1e3,
    formatC(bytes, format = "d", big.mark = ",")
  ))
}

cat(sprintf(
  "wickra-terminal R throughput - %s commands (median of 3)\n\n",
  formatC(ticks, format = "d", big.mark = ",")
))
cat(sprintf("%-18s%14s%14s%12s\n", "Command", "per second", "us/command", "payload"))
cat(strrep("-", 58), "\n", sep = "")
row("Tick", ticks, tick_ns, frame_bytes)
row("ListIndicators", 100, list_ns, catalogue_bytes)
cat(
  "\nOne command crosses the boundary once. Higher is better, and the numbers\n",
  "are machine-dependent -- compare bindings on one machine, never across two.\n",
  sep = ""
)
