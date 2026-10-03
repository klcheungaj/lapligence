// SV2009 10.3, 9.4.2, 10.6.2: an array beyond packed capacity reaches a
// continuous-assignment helper and an evaluated-event helper by descriptor
// (no per-cell expansion), and drives a force evaluator through its cells.
module tb;
  localparam int N = 65537;
  typedef logic [16:0] big_t [0:N-1];
  big_t a;
  logic [16:0] y, forced;
  function automatic logic [16:0] ends(input big_t v);
    logic [16:0] s;
    s = 0;
    for (int i = 0; i < 2; i++) s += v[i * (N - 1)];
    return s;
  endfunction
  assign y = ends(a);
  initial begin
    a[0] = 17'd1;
    a[N-1] = 17'd2;
    force forced = a[5] + a[N-1];
    #1 $display("continuous %0d", y);
    fork
      begin @(ends(a)) $display("event %0d %0t", ends(a), $time); end
      begin #1 a[5] = 17'd9; #1 a[N-1] = 17'd4; end
    join
    $display("continuous %0d", y);
    $display("force %0d", forced);
    $finish(0);
  end
endmodule
