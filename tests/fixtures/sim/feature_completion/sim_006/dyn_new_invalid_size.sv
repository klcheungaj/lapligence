// SIM-006 F2: a `new[]` size that is negative or has X or Z bits (SV 7.5.1,
// S07-D17). With index checks on (the default) each case stops the run at
// its `new[]`; with `--no-index-checks` the array becomes empty and a
// container warning names the size. `new[0]` is legal either way. `+case=N`
// runs case N only; no plusarg runs them all.
module tb;
  int d[];
  string s[];
  int c;
  int neg;
  longint big;
  logic [3:0] xs;
  logic [7:0] zs;
  function automatic bit run(int n);
    return c == 0 || c == n;
  endfunction
  initial begin
    if (!$value$plusargs("case=%d", c)) c = 0;
    neg = -1;
    big = -64'sd5000000000;
    xs = 4'bx01x;
    zs = 8'b0000_z001;
    if (run(1)) begin
      d = new[3];
      d = new[neg];
      $display("1 plain size %0d", d.size());
    end
    if (run(2)) begin
      d = '{4, 5, 6};
      d = new[neg](d);
      $display("2 copy size %0d", d.size());
    end
    if (run(3)) begin
      d = new[2];
      d = new[xs];
      $display("3 x size %0d", d.size());
    end
    if (run(4)) begin
      d = '{7, 8};
      d = new[zs](d);
      $display("4 z copy size %0d", d.size());
    end
    if (run(5)) begin
      s = '{"a", "b"};
      s = new[big];
      $display("5 string size %0d", s.size());
    end
    d = '{1, 2, 3};
    d = new[0];
    $display("zero size %0d", d.size());
    d = new[2](d);
    $display("regrow size %0d %0d", d.size(), d[1]);
    $finish(0);
  end
endmodule
