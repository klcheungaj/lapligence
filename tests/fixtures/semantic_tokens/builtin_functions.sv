// ST-40 to ST-43 (docs/semantic_tokens.md): built-in system functions and
// tasks and built-in methods in every context.
`define BIF_LOG2(n) $clog2(n)
package bif_pkg;
  localparam int PW = $clog2(16);
  typedef logic [$bits(int)-1:0] pw_t;
  function automatic int pf(int x);
    return $countones(x);
  endfunction
endpackage
class bif_cls;
  rand int r;
  int q[$];
  function void m();
    int n;
    n = q.size();
    srandom(3);
    void'(this.randomize() with { r < 2; });
  endfunction
  function int size();
    return 0;
  endfunction
endclass
module bif_top #(parameter int D = 8, parameter int AW = $clog2(D)) (
  input logic [$clog2(D)-1:0] addr,
  output logic [AW-1:0] y
);
  localparam real R = $ceil(2.5) + $floor(2.5) + $ln(2.0) + $sqrt(4.0);
  localparam int L = `BIF_LOG2(8);
  typedef logic [$bits(addr)-1:0] addr_t;
  typedef enum logic [1:0] {A, B, C} e_t;
  logic [7:0] v;
  int qq[$];
  string str;
  e_t e;
  mailbox mb = new();
  process p;
  bif_cls obj;
  int n;
  assign y = $signed(addr) + $unsigned(addr);
  if ($clog2(D) > 2) begin : g
    logic gx;
  end
  initial begin
    $display("%0d %0d", $size(qq), $countones(v));
    n = qq.size() - 1;
    n = qq.size;
    qq.push_back(1);
    qq = qq.unique();
    n = qq.and();
    n = qq.sum() with (item * 2);
    n = str.len();
    str = e.next().name();
    mb.put(1);
    p = process::self();
    void'(std::randomize(n) with { n < 3; });
    n = obj.size();
    n = bif_pkg::pf(3);
    $bif_unknown(1);
  end
endmodule
