// Top-level parameters of every data-type category, printed so command-line
// (`-G NAME=VALUE`) and llg.toml overrides can be checked against their types.
package po_pkg;
  typedef enum logic [1:0] {SLOW = 2'd0, MID = 2'd1, FAST = 2'd2} mode_t;
  typedef struct packed {
    logic [3:0] hi;
    logic [3:0] lo;
  } pair_t;
endpackage

module tb #(
  parameter string MSG = "default",
  parameter int COUNT = 1,
  parameter logic signed [7:0] OFFSET = 0,
  parameter logic [99:0] WIDE = 0,
  parameter logic [7:0] PATTERN = 0,
  parameter bit FLAG = 0,
  parameter real RATE = 0.0,
  parameter shortreal GAIN = 0.0,
  parameter realtime DELAY = 0.0,
  parameter ANY = 0,
  parameter po_pkg::mode_t MODE = po_pkg::SLOW,
  parameter po_pkg::pair_t PAIR = '0,
  parameter type ELEM = int
);
  localparam int FIXED = 3;
  ELEM elem = '1;
  initial begin
    $display("MSG=[%s] len=%0d", MSG, MSG.len());
    $display("COUNT=%0d OFFSET=%0d", COUNT, OFFSET);
    $display("WIDE=%0d", WIDE);
    $display("PATTERN=%b FLAG=%b", PATTERN, FLAG);
    $display("RATE=%f GAIN=%f DELAY=%f", RATE, GAIN, DELAY);
    $display("ANY=%0d bits=%0d", ANY, $bits(ANY));
    $display("MODE=%s PAIR=%h", MODE.name(), PAIR);
    $display("ELEM bits=%0d elem=%0d FIXED=%0d", $bits(ELEM), elem, FIXED);
  end
endmodule
