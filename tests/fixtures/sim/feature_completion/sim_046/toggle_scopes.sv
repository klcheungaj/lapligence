// SIM-046 the signals toggle coverage counts (decision S46-D14): every
// packed integral net and variable an instance declares, in its generate
// scopes (loop and conditional) included, and its ports; not an unpacked
// array, a real, a string, an event, a block's variable or a package's.
// Derivations: readme.md.
package pkg;
  logic [3:0] pv = 4'h0;
endpackage

module leaf(input logic [1:0] i);
  logic [2:0] q = 3'b000;
endmodule

module tb;
  typedef struct packed { logic [1:0] x; logic y; } st_t;
  logic [1:0] a = 2'b00;
  int unsigned u = 0;
  st_t st = '0;
  logic [7:0] arr [0:1];
  real r = 0.0;
  string s = "";
  event e;
  for (genvar g = 0; g < 2; g++) begin : gen
    logic [g:0] w = '0;
  end
  if (1) begin : cond
    wire c = a[0];
  end
  leaf l(a);
  initial begin
    automatic logic [7:0] local_v = 8'h00;
    $display("max tb %0d leaf %0d hier %0d",
             $coverage_get_max(`SV_COV_TOGGLE, `SV_COV_MODULE, tb),
             $coverage_get_max(`SV_COV_TOGGLE, `SV_COV_MODULE, l),
             $coverage_get_max(`SV_COV_TOGGLE, `SV_COV_HIER, tb));
    #1 a = 2'b11;
    u = 32'h0000_000f;
    st = '1;
    gen[0].w = 1'b1;
    gen[1].w = 2'b11;
    l.q = 3'b111;
    arr[0] = 8'hff;
    r = 1.0;
    s = "x";
    local_v = 8'hff;
    pkg::pv = 4'hf;
    -> e;
    #1 $display("covered tb %0d leaf %0d hier %0d",
                $coverage_get(`SV_COV_TOGGLE, `SV_COV_MODULE, tb),
                $coverage_get(`SV_COV_TOGGLE, `SV_COV_MODULE, l),
                $coverage_get(`SV_COV_TOGGLE, `SV_COV_HIER, tb));
    $finish;
  end
endmodule
