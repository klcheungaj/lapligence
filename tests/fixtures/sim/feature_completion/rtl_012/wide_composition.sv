// RTL-012 composition: 70-bit nets (two limbs) mix an omitted pull1 input,
// gate arrays with legal per-instance strengths, wired resolution and plain
// competing continuous drivers. Every bit is checked by the scalar oracle.
`unconnected_drive pull1
module child #(parameter int W = 70) (input wire [W-1:0] bus, input wand [W-1:0] wbus,
                                      input logic [W-1:0] d, input logic [W-1:0] e);
  bufif1 (strong0, weak1) g[W-1:0] (bus, d, e);
  pulldown (weak0) pd[W-1:0] (wbus);
  buf (pull0, pull1) bw[W-1:0] (wbus, d);
endmodule
`nounconnected_drive
module tb;
  localparam int W = 70;
  logic [W-1:0] d, e, f;
  wire [W-1:0] p;
  assign p = d;
  assign p = f;
  child #(W) c(.d(d), .e(e));
  function automatic logic val(int k);
    case (k % 4)
      0: return 1'b0;
      1: return 1'b1;
      2: return 1'bx;
      default: return 1'bz;
    endcase
  endfunction
  initial begin
    for (int step = 0; step < 3; step++) begin
      for (int k = 0; k < W; k++) begin
        d[k] = val(k + step);
        e[k] = val(k / 4 + step);
        f[k] = val(k / 16 + 2 * step);
      end
      #1;
      $display("bus %b", c.bus);
      $display("bus %v", c.bus);
      $display("wbus %b", c.wbus);
      $display("wbus %v", c.wbus);
      $display("p %b", p);
      $display("p %v", p);
    end
    $finish(0);
  end
endmodule
