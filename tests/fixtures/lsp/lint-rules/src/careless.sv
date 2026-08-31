// llg-lsp-fixture: lint-rules/src/careless.sv
// Exercises three careless-mistake lint rules end to end:
// - implicit-net:              `data_bus` is connected but never declared
// - case-default-missing:      edge-sensitive `case` without a default arm
// - comparison-width-mismatch: 4-bit `sel` compared against an 8-bit literal
module careless_child (
  input  wire [7:0] data,
  output wire       ack
);
  assign ack = |data;
endmodule

module careless_top (
  input  wire [3:0] sel,
  input  wire       clk,
  output wire [7:0] q,
  output wire       strobe_err
);
  reg [7:0] shreg;

  // `data_bus` is never declared: Surelog promotes it to a one-bit wire at
  // this connection, leaving the child input undriven.
  careless_child u_child (.data(data_bus), .ack(strobe_err));

  always_ff @(posedge clk) begin
    if (sel == 8'h05)
      shreg <= {sel, 4'b0000};
    else begin
      case (sel)
        4'd0: shreg <= 8'h00;
        4'd1: shreg <= 8'hff;
      endcase
    end
  end

  assign q = shreg;
endmodule
