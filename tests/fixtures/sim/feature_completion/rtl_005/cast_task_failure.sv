// SV2009 6.24.2: a failed task-form $cast is a run-time error that leaves
// its destination unchanged; execution continues after the report.
module tb;
  typedef enum logic [2:0] { A0 = 3'd0, A2 = 3'd2, A5 = 3'd5 } e_t;
  localparam logic [31:0] STDERR = 32'h8000_0002;
  e_t e;
  logic [31:0] value;
  initial begin
    e = A2;
    value = 32'd5;
    $cast(e, value);
    $fdisplay(STDERR, "A %0d", e);
    value = 32'd7;
    $cast(e, value);
    $fdisplay(STDERR, "B %0d", e);
    $finish(0);
  end
endmodule
