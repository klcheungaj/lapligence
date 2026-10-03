// SV2009 6.8, 6.21, 7.4, 10.5, 10.9.1, 11.13: oversized fixed arrays (beyond
// one packed payload) initialize through descriptor transport, never one
// flattened value. Static declaration initializers of package, module,
// function-static and block-static arrays run once before any process and do
// not generate events; later initializers read their values. Automatic block
// and function arrays initialize on every entry, including entries that
// suspend before using the array.
package big_pkg;
  typedef logic [16:0] big_t [0:65536];
  int fillv = 9;
  function automatic logic [16:0] seedv();
    return 17'(fillv + 1);
  endfunction
  big_t big = '{0: seedv(), 65536: 17'h1ffff, default: 17'(fillv)};
endpackage

module tb;
  import big_pkg::big_t;
  let cp(x) = x;
  big_t copy = big_pkg::big;
  big_t via_let = cp(copy);
  logic [16:0] first = big_pkg::big[0];
  logic [16:0] last = copy[65536];
  logic [16:0] mid = via_let[100];
  int hits = 0;
  logic clk = 0;
  int total = 0;
  always @(copy[3]) hits++;
  always #5 clk = ~clk;
  function automatic int probe(int i);
    static logic [16:0] table_s [0:65536] = '{default: 17'h7};
    table_s[i] = table_s[i] + 1;
    return table_s[i];
  endfunction
  function automatic int fresh(int i);
    logic [16:0] scratch [0:65536] = '{default: 17'h7};
    scratch[i] = scratch[i] + 1;
    return scratch[i];
  endfunction
  int p1 = probe(3);
  int f1 = fresh(3);
  always @(posedge clk) begin
    static logic [16:0] hist [0:65536] = '{default: 17'h2};
    automatic logic [16:0] work [0:65536] = '{default: 17'h10};
    hist[0] = hist[0] + 1;
    work[1] = work[1] + hist[0];
    #1;
    total = total + work[1] + work[2];
  end
  initial begin
    $display("init %h %h %h %h %0d", first, last, mid, copy[0], hits);
    $display("calls %0d %0d %0d %0d", p1, probe(3), f1, fresh(3));
    for (int k = 0; k < 2; k++) begin
      automatic logic [16:0] w [0:65536] = '{default: 17'(k + 3)};
      #2;
      w[k] = w[k] + 1;
      $display("block %0d %0d %0d", k, w[k], w[5]);
    end
    copy[3] = 17'h3;
    #30;
    $display("final %0d %0d %b", total, hits, cp(copy) == via_let);
    $finish(0);
  end
endmodule
